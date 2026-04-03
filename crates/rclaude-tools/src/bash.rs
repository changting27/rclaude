use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::process::Command;

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

const DEFAULT_TIMEOUT_MS: u64 = 120_000;
const MAX_TIMEOUT_MS: u64 = 600_000;
const MAX_OUTPUT_BYTES: usize = 1_000_000;

const DESCRIPTION: &str = "Executes a given bash command and returns its output.\n\n\
The working directory persists between commands, but shell state does not.\n\n\
IMPORTANT: Avoid using this tool to run `find`, `grep`, `cat`, `head`, `tail`, `sed`, `awk`, \
or `echo` commands. Instead, use the appropriate dedicated tool.";

/// Dangerous commands that are always blocked (checked at token level).
const BLOCKED_COMMANDS: &[&str] = &["mkfs", "fdisk", "parted", "wipefs"];

/// Dangerous argument patterns (rm with recursive force on root).
const DANGEROUS_RM_TARGETS: &[&str] = &[
    "/", "/*", "/home", "/etc", "/usr", "/var", "/boot", "/sys", "/proc",
];

/// Commands that warrant a warning.
const WARN_COMMANDS: &[(&str, &[&str])] = &[
    (
        "git",
        &[
            "push --force",
            "push -f",
            "reset --hard",
            "checkout -- .",
            "clean -f",
            "clean -fd",
            "branch -D",
        ],
    ),
    ("rm", &["-rf", "-fr"]),
    ("chmod", &["-R 777"]),
    ("dd", &["if="]),
];

/// Flags that warrant a warning on any command.
const WARN_FLAGS: &[&str] = &["--no-verify", "--force", "--hard"];

/// Read-only commands that are safe to auto-approve in Auto mode.
const READ_ONLY_COMMANDS: &[&str] = &[
    // File listing / info
    "ls",
    "ll",
    "la",
    "dir",
    "vdir",
    "pwd",
    "tree",
    "file",
    "stat",
    "readlink",
    "realpath",
    "dirname",
    "basename",
    "pathchk",
    // File content viewing
    "cat",
    "head",
    "tail",
    "less",
    "more",
    "tac",
    "rev",
    "nl",
    "wc",
    // Text processing (read-only)
    "echo",
    "printf",
    "sort",
    "uniq",
    "cut",
    "tr",
    "paste",
    "column",
    "expand",
    "unexpand",
    "fold",
    "fmt",
    "pr",
    "comm",
    "join",
    "csplit",
    // Search
    "grep",
    "egrep",
    "fgrep",
    "find",
    "locate",
    "mlocate",
    "which",
    "whereis",
    "whence",
    "type",
    "command",
    "hash",
    // Diff / compare
    "diff",
    "diff3",
    "sdiff",
    "cmp",
    // Encoding / hashing
    "md5sum",
    "sha1sum",
    "sha256sum",
    "sha512sum",
    "shasum",
    "cksum",
    "b2sum",
    "base64",
    "xxd",
    "hexdump",
    "od",
    "strings",
    // System info
    "whoami",
    "hostname",
    "uname",
    "uptime",
    "date",
    "cal",
    "timedatectl",
    "id",
    "groups",
    "who",
    "w",
    "last",
    "finger",
    "df",
    "du",
    "free",
    "vmstat",
    "iostat",
    "nproc",
    "lscpu",
    "lsmem",
    "lsblk",
    "lspci",
    "lsusb",
    "lsmod",
    // Process info
    "ps",
    "top",
    "htop",
    "pgrep",
    "pidof",
    "lsof",
    "fuser",
    "pstree",
    // Network info (read-only)
    "ifconfig",
    "ip",
    "ss",
    "netstat",
    "route",
    "host",
    "dig",
    "nslookup",
    "ping",
    "traceroute",
    "tracepath",
    // Environment
    "env",
    "printenv",
    "locale",
    "getconf",
    // Misc read-only
    "test",
    "[",
    "true",
    "false",
    "yes",
    "seq",
    "shuf",
    "tee",
    "xargs",
    "time",
    "timeout",
    // Search tools
    "rg",
    "fd",
    "fdfind",
    "ag",
    "ack",
    "jq",
    "yq",
    // Documentation
    "man",
    "info",
    "help",
    "apropos",
];

/// Git read-only subcommands.
const GIT_READ_ONLY_SUBCOMMANDS: &[&str] = &[
    "status",
    "log",
    "diff",
    "show",
    "branch",
    "tag",
    "remote",
    "describe",
    "rev-parse",
    "rev-list",
    "ls-files",
    "ls-tree",
    "cat-file",
    "name-rev",
    "shortlog",
    "blame",
    "grep",
    "config",
    "stash list",
    "reflog",
];

/// Check if a command is read-only (safe for auto-approve).
/// - Split compound commands and check each segment
/// - Reject commands with `$` variable expansion (runtime value unknown)
/// - Reject commands with dangerous flags (--exec, -o/--output, etc.)
/// - Special git safety checks (-c, --exec-path, --config-env)
pub fn is_read_only_command(command: &str) -> bool {
    let segments: Vec<&str> = command
        .split(['|', ';'])
        .flat_map(|s| s.split("&&"))
        .flat_map(|s| s.split("||"))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    segments.iter().all(|seg| is_segment_read_only(seg))
}

/// Dangerous flags that should never be auto-approved regardless of command.
const DANGEROUS_FLAGS: &[&str] = &[
    "--exec",
    "--exec-batch",
    "-exec",
    "-execdir",
    "--exec-path",
    "--config-env",
    "--upload-pack",
    "--run-tests",
    "--from-file",
    "--rawfile",
    "--slurpfile",
    "-o",
    "--output",
    "--output-file",
    "-delete",
    "-fprint",
    "-fprint0",
    "-fls",
    "-fprintf",
    "-ok",
    "-okdir",
];

/// Flags dangerous specifically for git commands.
const GIT_DANGEROUS_PATTERNS: &[&str] = &["-c", "--exec-path", "--config-env"];

/// Check if a single command segment is read-only.
fn is_segment_read_only(seg: &str) -> bool {
    // Strip trailing stderr redirection
    let seg = seg.strip_suffix("2>&1").unwrap_or(seg).trim();

    let tokens = match shell_words::split(seg) {
        Ok(t) => t,
        Err(_) => return false,
    };
    if tokens.is_empty() {
        return true;
    }

    // SECURITY: Reject any token containing $ (variable expansion).
    // We can't know runtime values, so can't verify read-only safety.
    for token in &tokens[1..] {
        if token.contains('$') {
            return false;
        }
        // Reject brace expansion: {a,b} or {1..5}
        if token.contains('{') && (token.contains(',') || token.contains("..")) {
            return false;
        }
    }

    // Reject backtick command substitution
    if seg.contains('`') {
        return false;
    }

    // Check for dangerous flags in any command
    for token in &tokens[1..] {
        let lower = token.to_lowercase();
        for flag in DANGEROUS_FLAGS {
            if lower == *flag || lower.starts_with(&format!("{flag}=")) {
                return false;
            }
        }
    }

    let cmd = tokens[0].to_lowercase();

    // Git with read-only subcommand + safety checks
    if cmd == "git" {
        return is_git_read_only(&tokens);
    }

    // Direct read-only command
    if READ_ONLY_COMMANDS.contains(&cmd.as_str()) {
        // Additional per-command checks
        return is_command_flags_safe(&cmd, &tokens);
    }

    // ripgrep / fd — read-only but check flags
    if matches!(cmd.as_str(), "rg" | "fd" | "fdfind" | "ag") {
        // rg --pre=cmd executes arbitrary commands
        for token in &tokens[1..] {
            if token.starts_with("--pre") {
                return false;
            }
        }
        // fd -x/--exec executes commands
        if cmd == "fd" || cmd == "fdfind" {
            for token in &tokens[1..] {
                if token == "-x" || token == "-X" || token.starts_with("--exec") {
                    return false;
                }
                // fd -l/--list-details internally executes ls
                if token == "-l" || token == "--list-details" {
                    return false;
                }
            }
        }
        return true;
    }

    // docker ps / docker images
    if cmd == "docker" {
        if let Some(sub) = tokens.get(1) {
            return matches!(sub.as_str(), "ps" | "images");
        }
    }

    false
}

/// Check git command is read-only with security hardening.
fn is_git_read_only(tokens: &[String]) -> bool {
    if tokens.len() < 2 {
        return false;
    }

    // SECURITY: Block git -c (arbitrary config, can execute commands)
    // Block git --exec-path (path manipulation → code execution)
    // Block git --config-env (config injection via env vars)
    for token in &tokens[1..] {
        for pattern in GIT_DANGEROUS_PATTERNS {
            if token == *pattern || token.starts_with(&format!("{pattern}=")) {
                return false;
            }
        }
    }

    let sub = tokens[1].as_str();

    // git config is only safe with --get/--list (read operations)
    if sub == "config" {
        return tokens[2..]
            .iter()
            .any(|t| t == "--get" || t == "--get-all" || t == "--list" || t == "-l");
    }

    // git stash is only safe with "list"
    if sub == "stash" {
        return tokens.get(2).is_some_and(|t| t == "list");
    }

    // git ls-remote: reject URLs (data exfiltration)
    if sub == "ls-remote" {
        for token in &tokens[2..] {
            if !token.starts_with('-')
                && (token.contains("://") || token.contains('@') || token.contains(':'))
            {
                return false;
            }
        }
    }

    GIT_READ_ONLY_SUBCOMMANDS.contains(&sub)
}

/// Per-command flag safety checks for specific commands.
fn is_command_flags_safe(cmd: &str, tokens: &[String]) -> bool {
    match cmd {
        // find: block -exec, -delete, -fprint etc. (already in DANGEROUS_FLAGS)
        // but also block unescaped parentheses
        "find" => {
            for token in &tokens[1..] {
                if token == "(" || token == ")" {
                    return false;
                }
            }
            true
        }
        // xargs: only safe with safe target commands
        "xargs" => is_xargs_safe(tokens),
        // sed: delegate to existing sed validation
        "sed" => crate::bash_sed::sed_command_is_allowed(&tokens.join(" "), false),
        // ps: block 'e' flag (shows env vars of all processes)
        "ps" => {
            for token in &tokens[1..] {
                if !token.starts_with('-') && token.contains('e') {
                    return false;
                }
            }
            true
        }
        _ => true,
    }
}

/// Safe target commands for xargs in read-only mode.
const XARGS_SAFE_TARGETS: &[&str] = &[
    "echo",
    "cat",
    "head",
    "tail",
    "wc",
    "grep",
    "egrep",
    "fgrep",
    "file",
    "stat",
    "basename",
    "dirname",
    "realpath",
    "readlink",
    "ls",
    "du",
    "md5sum",
    "sha256sum",
    "sha1sum",
];

/// Check if xargs command only invokes safe targets.
fn is_xargs_safe(tokens: &[String]) -> bool {
    // Find the target command (first non-flag token after xargs flags)
    let mut i = 1;
    while i < tokens.len() {
        let t = &tokens[i];
        if t.starts_with('-') {
            // Skip flag and its argument if applicable
            if matches!(t.as_str(), "-I" | "-n" | "-P" | "-L" | "-s" | "-E" | "-d") {
                i += 2; // skip flag + value
            } else {
                i += 1;
            }
        } else {
            // This is the target command
            return XARGS_SAFE_TARGETS.contains(&t.as_str());
        }
    }
    // No target command found — xargs with no command defaults to echo, which is safe
    true
}

pub struct BashTool;

/// Comprehensive permission check for a bash command.
/// Integrates tree-sitter AST parsing with permission rules.
/// Flow:
/// 1. Parse with tree-sitter AST
/// 2. Security pre-checks (control chars, substitution, etc.)
/// 3. If read-only → auto-allow
/// 4. Check against permission rules
/// 5. Path validation for write commands
/// 6. Sed validation
/// 7. Fallback → ask user
pub fn check_bash_permission(
    command: &str,
    mode: rclaude_core::permissions::PermissionMode,
    rules: &[rclaude_core::permissions::PermissionRule],
    cwd: &std::path::Path,
) -> BashPermissionResult {
    // 1. Parse with tree-sitter AST
    let ast_result = crate::bash_ast::parse_for_security(command);
    let ast_succeeded = matches!(ast_result, crate::bash_ast::ParseResult::Simple(_));

    // 2. Security pre-checks
    let security = analyze_command_security(command);
    if let Some(blocked) = &security.blocked {
        return BashPermissionResult::Deny(blocked.clone());
    }

    // 3. Bypass mode
    if mode == rclaude_core::permissions::PermissionMode::BypassPermissions {
        return BashPermissionResult::Allow;
    }

    // 4. Plan mode — only read-only
    if mode == rclaude_core::permissions::PermissionMode::Plan {
        if is_read_only_command(command) {
            return BashPermissionResult::Allow;
        }
        return BashPermissionResult::Deny("Write commands not allowed in plan mode".into());
    }

    // 5. Check explicit permission rules
    let rule_result =
        rclaude_core::permissions::check_permission_with_rules("Bash", mode, rules, Some(command));
    match &rule_result {
        rclaude_core::permissions::PermissionResult::Allowed => return BashPermissionResult::Allow,
        rclaude_core::permissions::PermissionResult::Denied(reason) => {
            return BashPermissionResult::Deny(reason.clone())
        }
        _ => {} // NeedApproval — continue checking
    }

    // 6. Read-only commands auto-allow (matching step 7 in original)
    if is_read_only_command(command) {
        return BashPermissionResult::Allow;
    }

    // 7. If AST parsed successfully, use it for deeper analysis
    if let crate::bash_ast::ParseResult::Simple(ref commands) = ast_result {
        // Check path validation for write commands
        for cmd in commands {
            if cmd.argv.is_empty() {
                continue;
            }
            let base = &cmd.argv[0];
            if let Some(op_type) = crate::bash_path::command_op_type(base) {
                if op_type == crate::bash_path::FileOpType::Write
                    || op_type == crate::bash_path::FileOpType::Create
                {
                    let args: Vec<String> = cmd.argv[1..].to_vec();
                    let validation = crate::bash_path::validate_command_paths(base, &args, cwd);
                    if !validation.allowed {
                        return BashPermissionResult::Ask(validation.reason);
                    }
                }
            }
        }

        // Check sed commands
        for cmd in commands {
            if cmd.argv.first().is_some_and(|a| a == "sed")
                && !crate::bash_sed::sed_command_is_allowed(&cmd.text, false)
            {
                return BashPermissionResult::Ask(
                    "sed command requires approval (potentially dangerous operations)".into(),
                );
            }
        }
    }

    // 8. Auto mode — allow non-dangerous commands (static rules)
    if mode == rclaude_core::permissions::PermissionMode::Auto
        && ast_succeeded
        && security.warnings.is_empty()
    {
        return BashPermissionResult::Allow;
    }

    // 8b. Auto mode — LLM classifier fallback for uncertain commands
    if mode == rclaude_core::permissions::PermissionMode::Auto && !security.warnings.is_empty() {
        // Try LLM classification if API key is available
        if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
            if !api_key.is_empty() {
                let model = std::env::var("ANTHROPIC_DEFAULT_SONNET_MODEL")
                    .or_else(|_| std::env::var("CLAUDE_MODEL"))
                    .unwrap_or_else(|_| "claude-sonnet-4-20250514".into());
                // Use tokio runtime if available, otherwise skip
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    let cmd = command.to_string();
                    let key = api_key.clone();
                    if let Ok(allowed) = handle.block_on(
                        crate::bash_classifier::classify_bash_command(&cmd, &key, &model),
                    ) {
                        if allowed {
                            return BashPermissionResult::Allow;
                        }
                    }
                }
            }
        }
    }

    // 9. Fallback — ask user
    let reason = if !security.warnings.is_empty() {
        security.warnings.join("; ")
    } else {
        "This command requires approval".into()
    };
    BashPermissionResult::Ask(reason)
}

/// Result of bash permission check.
#[derive(Debug, Clone)]
pub enum BashPermissionResult {
    Allow,
    Deny(String),
    Ask(String),
}

/// Security analysis result.
struct SecurityCheck {
    blocked: Option<String>,
    warnings: Vec<String>,
}

// ── Security patterns ──

/// Command substitution patterns that require user approval.
const COMMAND_SUBSTITUTION_PATTERNS: &[(&str, &str)] = &[
    (r"<(", "process substitution <()"),
    (r">(", "process substitution >()"),
    (r"=(", "Zsh process substitution =()"),
    ("$(", "$() command substitution"),
    ("${", "${} parameter substitution"),
    ("$[", "$[] legacy arithmetic expansion"),
];

/// Zsh dangerous commands.
const ZSH_DANGEROUS_COMMANDS: &[&str] = &[
    "zmodload", "emulate", "sysopen", "sysread", "syswrite", "sysseek", "zpty", "ztcp", "zsocket",
    "zf_rm", "zf_mv", "zf_ln", "zf_chmod",
];

/// Validate command substitution patterns.
fn validate_command_substitution(command: &str) -> Option<String> {
    // Unescaped backticks
    let bytes = command.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'`' && (i == 0 || bytes[i - 1] != b'\\') {
            return Some("Command contains backticks (`) for command substitution".into());
        }
    }
    for (pattern, desc) in COMMAND_SUBSTITUTION_PATTERNS {
        if command.contains(pattern) {
            return Some(format!("Command contains {desc}"));
        }
    }
    None
}

/// Validate dangerous variable usage in redirections/pipes.
fn validate_dangerous_variables(command: &str) -> Option<String> {
    let re_var_redir = regex::Regex::new(r"[<>|]\s*\$[A-Za-z_]").unwrap();
    let re_redir_var = regex::Regex::new(r"\$[A-Za-z_][A-Za-z0-9_]*\s*[|<>]").unwrap();
    if re_var_redir.is_match(command) || re_redir_var.is_match(command) {
        return Some(
            "Command contains variables in dangerous contexts (redirections or pipes)".into(),
        );
    }
    None
}

/// Validate input/output redirection.
fn validate_redirections(command: &str) -> Option<String> {
    // Strip quoted strings to avoid false positives
    let unquoted = strip_quoted_strings(command);
    if unquoted.contains('<') && !unquoted.contains("<<") {
        return Some(
            "Command contains input redirection (<) which could read sensitive files".into(),
        );
    }
    // Output redirection (but allow heredocs <<)
    if regex::Regex::new(r"[^<]>|^>").unwrap().is_match(&unquoted) {
        return Some(
            "Command contains output redirection (>) which could write to arbitrary files".into(),
        );
    }
    None
}

/// Validate newline injection.
fn validate_newlines(command: &str) -> Option<String> {
    if command.contains('\n') || command.contains('\r') {
        return Some("Command contains newlines which could hide malicious commands".into());
    }
    None
}

/// Validate IFS injection.
fn validate_ifs_injection(command: &str) -> Option<String> {
    if command.contains("IFS=") {
        return Some("Command modifies IFS which could alter command parsing".into());
    }
    None
}

/// Validate /proc/*/environ access.
fn validate_proc_environ(command: &str) -> Option<String> {
    if regex::Regex::new(r"/proc/[^/]+/environ")
        .unwrap()
        .is_match(command)
    {
        return Some("Command accesses /proc/*/environ which could leak secrets".into());
    }
    None
}

/// Validate control characters.
fn validate_control_characters(command: &str) -> Option<String> {
    for ch in command.chars() {
        if ch.is_control() && ch != '\t' && ch != '\n' && ch != '\r' {
            return Some(format!(
                "Command contains control character U+{:04X}",
                ch as u32
            ));
        }
    }
    None
}

/// Validate unicode whitespace (non-ASCII spaces that could hide commands).
fn validate_unicode_whitespace(command: &str) -> Option<String> {
    for ch in command.chars() {
        if ch.is_whitespace() && !ch.is_ascii() {
            return Some(format!(
                "Command contains non-ASCII whitespace U+{:04X}",
                ch as u32
            ));
        }
    }
    None
}

/// Validate Zsh dangerous commands.
fn validate_zsh_commands(tokens: &[String]) -> Option<String> {
    if let Some(cmd) = tokens.first() {
        let lower = cmd.to_lowercase();
        if ZSH_DANGEROUS_COMMANDS.contains(&lower.as_str()) {
            return Some(format!("Blocked: '{lower}' is a dangerous Zsh command"));
        }
    }
    None
}

/// Strip single and double quoted strings from command for pattern matching.
fn strip_quoted_strings(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_single = false;
    let mut in_double = false;
    let mut prev = '\0';
    for ch in s.chars() {
        match ch {
            '\'' if !in_double && prev != '\\' => in_single = !in_single,
            '"' if !in_single && prev != '\\' => in_double = !in_double,
            _ if !in_single && !in_double => result.push(ch),
            _ => {}
        }
        prev = ch;
    }
    result
}

/// Analyze a command for security using shell-words tokenization.
/// Multi-layer validation pipeline.
fn analyze_command_security(command: &str) -> SecurityCheck {
    let mut result = SecurityCheck {
        blocked: None,
        warnings: Vec::new(),
    };

    // ── Pre-tokenization checks (raw string) ──

    // Fork bomb detection
    if command.contains(":(){ :|:& };:") || command.contains(":(){ :|:&};:") {
        result.blocked = Some("Blocked: fork bomb detected".to_string());
        return result;
    }

    // Newline injection
    if let Some(msg) = validate_newlines(command) {
        result.warnings.push(msg);
    }

    // Control characters
    if let Some(msg) = validate_control_characters(command) {
        result.blocked = Some(msg);
        return result;
    }

    // Unicode whitespace
    if let Some(msg) = validate_unicode_whitespace(command) {
        result.blocked = Some(msg);
        return result;
    }

    // Command substitution (backticks, $(), ${}, etc.)
    if let Some(msg) = validate_command_substitution(command) {
        result.warnings.push(msg);
    }

    // Dangerous variables in redirections/pipes
    if let Some(msg) = validate_dangerous_variables(command) {
        result.warnings.push(msg);
    }

    // Redirections
    if let Some(msg) = validate_redirections(command) {
        result.warnings.push(msg);
    }

    // IFS injection
    if let Some(msg) = validate_ifs_injection(command) {
        result.warnings.push(msg);
    }

    // /proc/*/environ access
    if let Some(msg) = validate_proc_environ(command) {
        result.blocked = Some(msg);
        return result;
    }

    // ── Per-segment tokenized checks ──

    let segments: Vec<&str> = command
        .split(['|', ';'])
        .flat_map(|s| s.split("&&"))
        .flat_map(|s| s.split("||"))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    for segment in &segments {
        let tokens = match shell_words::split(segment) {
            Ok(t) => t,
            Err(_) => {
                check_substring_fallback(segment, &mut result);
                continue;
            }
        };

        if tokens.is_empty() {
            continue;
        }

        let cmd = tokens[0].to_lowercase();

        // Zsh dangerous commands
        if let Some(msg) = validate_zsh_commands(&tokens) {
            result.blocked = Some(msg);
            return result;
        }

        // Blocked commands (prefix match for mkfs.ext4 etc.)
        if BLOCKED_COMMANDS
            .iter()
            .any(|bc| cmd == *bc || cmd.starts_with(&format!("{bc}.")))
        {
            result.blocked = Some(format!("Blocked: '{cmd}' is a destructive system command"));
            return result;
        }

        // rm with dangerous targets
        if cmd == "rm" {
            let has_recursive = tokens.iter().any(|t| {
                t == "-rf"
                    || t == "-fr"
                    || t == "-r"
                    || (t.starts_with('-') && t.contains('r') && t.contains('f'))
            });
            if has_recursive {
                for token in &tokens[1..] {
                    if DANGEROUS_RM_TARGETS.contains(&token.as_str()) {
                        result.blocked = Some(format!(
                            "Blocked: 'rm -rf {token}' would destroy critical system files"
                        ));
                        return result;
                    }
                }
                result
                    .warnings
                    .push("Warning: rm with recursive force flag".to_string());
            }
        }

        // Warn patterns
        for (warn_cmd, patterns) in WARN_COMMANDS {
            if cmd == *warn_cmd {
                let full = tokens[1..].join(" ").to_lowercase();
                for pat in *patterns {
                    if full.contains(pat) {
                        result
                            .warnings
                            .push(format!("Warning: '{cmd}' with risky pattern '{pat}'"));
                    }
                }
            }
        }

        // Dangerous flags on any command
        for token in &tokens {
            let lower_token = token.to_lowercase();
            for flag in WARN_FLAGS {
                if lower_token == *flag {
                    result
                        .warnings
                        .push(format!("Warning: '{cmd}' with flag '{flag}'"));
                }
            }
        }
    }

    result
}

/// Fallback for commands that can't be tokenized.
fn check_substring_fallback(segment: &str, result: &mut SecurityCheck) {
    let lower = segment.to_lowercase();
    if lower.contains("rm -rf /") && !lower.contains("rm -rf ./") {
        result.blocked = Some("Blocked: dangerous rm -rf pattern".to_string());
    }
    if lower.contains("> /dev/sd") {
        result.blocked = Some("Blocked: write to block device".to_string());
    }
}

fn truncate_output(s: &str, max_bytes: usize) -> (String, bool) {
    if s.len() <= max_bytes {
        (s.to_string(), false)
    } else {
        // Find a valid UTF-8 boundary at or before max_bytes
        let safe_end = floor_char_boundary(s, max_bytes);
        // Then find last newline to avoid breaking mid-line
        let end = s[..safe_end].rfind('\n').unwrap_or(safe_end);
        (
            format!(
                "{}\n... (output truncated, {} bytes total)",
                &s[..end],
                s.len()
            ),
            true,
        )
    }
}

/// Find the largest byte index <= `index` that is a valid char boundary.
fn floor_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    let mut i = index;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "Bash"
    }

    fn description(&self) -> &str {
        DESCRIPTION
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute"
                },
                "description": {
                    "type": "string",
                    "description": "Clear description of what this command does"
                },
                "timeout": {
                    "type": "number",
                    "description": "Optional timeout in milliseconds (max 600000)"
                },
                "run_in_background": {
                    "type": "boolean",
                    "description": "Set to true to run in the background"
                }
            },
            "required": ["command"]
        }))
        .expect("valid schema")
    }

    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing required field: command".into()))?;

        // Bash-specific permission check (AST + security + rules)
        let rules = if let Some(ref state) = ctx.app_state {
            let s = state.read().await;
            rclaude_core::permissions::rules_from_config(&s.config)
        } else {
            vec![]
        };
        let perm = check_bash_permission(command, ctx.permission_mode, &rules, &ctx.cwd);
        match perm {
            BashPermissionResult::Deny(reason) => {
                return Ok(ToolResult::error(format!("Permission denied: {reason}")));
            }
            BashPermissionResult::Ask(reason) => {
                if !rclaude_core::permissions::prompt_user_permission(&format!(
                    "Bash: {command}\n{reason}"
                )) {
                    return Ok(ToolResult::error("Permission denied by user"));
                }
            }
            BashPermissionResult::Allow => {}
        }

        // Security analysis (for warnings only — blocking already handled above)
        let security = analyze_command_security(command);
        let warnings = security.warnings;

        let timeout_ms = input
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_MS)
            .min(MAX_TIMEOUT_MS);

        let _run_in_background = input
            .get("run_in_background")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let timeout = Duration::from_millis(timeout_ms);

        let mut cmd = Command::new("sh");
        cmd.arg("-c")
            .arg(command)
            .current_dir(&ctx.cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| RclaudeError::Tool(format!("Failed to execute command: {e}")))?;

        // U07: Stream stdout in real-time
        let stdout_pipe = child.stdout.take();
        let stderr_pipe = child.stderr.take();

        let stdout_handle = tokio::spawn(async move {
            let mut output = String::new();
            if let Some(pipe) = stdout_pipe {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut reader = BufReader::new(pipe);
                let mut line = String::new();
                while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                    // Only stream to stderr in interactive mode
                    if atty::is(atty::Stream::Stderr) {
                        eprint!("{}", line);
                    }
                    output.push_str(&line);
                    line.clear();
                    if output.len() > MAX_OUTPUT_BYTES {
                        break;
                    }
                }
            }
            output
        });

        let stderr_handle = tokio::spawn(async move {
            let mut output = String::new();
            if let Some(pipe) = stderr_pipe {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut reader = BufReader::new(pipe);
                let mut line = String::new();
                while reader.read_line(&mut line).await.unwrap_or(0) > 0 {
                    output.push_str(&line);
                    line.clear();
                    if output.len() > MAX_OUTPUT_BYTES / 2 {
                        break;
                    }
                }
            }
            output
        });

        let status = tokio::time::timeout(timeout, child.wait())
            .await
            .map_err(|_| {
                RclaudeError::Timeout(format!(
                    "Command timed out after {}ms: {}",
                    timeout_ms,
                    &command[..command.len().min(100)]
                ))
            })?
            .map_err(|e| RclaudeError::Tool(format!("Failed to wait for command: {e}")))?;

        let exit_code = status.code().unwrap_or(-1);
        let stdout = stdout_handle.await.unwrap_or_default();
        let stderr = stderr_handle.await.unwrap_or_default();

        let mut result = String::new();

        // Add warnings
        for w in &warnings {
            result.push_str(w);
            result.push('\n');
        }

        let (stdout_truncated, _) = truncate_output(&stdout, MAX_OUTPUT_BYTES);
        let (stderr_truncated, _) = truncate_output(&stderr, MAX_OUTPUT_BYTES / 2);

        if !stdout_truncated.is_empty() {
            result.push_str(&stdout_truncated);
        }

        if !stderr_truncated.is_empty() {
            if !result.is_empty() && !result.ends_with('\n') {
                result.push('\n');
            }
            result.push_str(&stderr_truncated);
        }

        if result.is_empty() {
            result = format!("(Bash completed with no output, exit code {exit_code})");
        }

        if exit_code != 0 {
            // Non-zero exit: include exit code in error result
            if !result.contains("exit code") {
                result.push_str(&format!("\n(exit code: {exit_code})"));
            }
            Ok(ToolResult::error(result))
        } else {
            Ok(ToolResult::text(result))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dangerous_pattern_detection() {
        let r = analyze_command_security("rm -rf /");
        assert!(r.blocked.is_some());

        let r = analyze_command_security("rm -rf /*");
        assert!(r.blocked.is_some());

        let r = analyze_command_security("ls -la");
        assert!(r.blocked.is_none());

        // Fork bomb
        let r = analyze_command_security(":(){ :|:& };:");
        assert!(r.blocked.is_some());

        // Blocked command
        let r = analyze_command_security("mkfs.ext4 /dev/sda1");
        assert!(r.blocked.is_some());

        // Quoted args should still be caught
        let r = analyze_command_security("rm -rf '/'");
        assert!(r.blocked.is_some());
    }

    #[test]
    fn test_warn_patterns() {
        let r = analyze_command_security("git push --force origin main");
        assert!(!r.warnings.is_empty());

        let r = analyze_command_security("git add .");
        assert!(
            r.warnings.is_empty(),
            "git add should have no warnings: {:?}",
            r.warnings
        );

        let r = analyze_command_security("rm -rf ./node_modules");
        assert!(!r.warnings.is_empty());
        assert!(r.blocked.is_none());
    }

    #[test]
    fn test_command_substitution_detection() {
        let r = analyze_command_security("echo `whoami`");
        assert!(r.warnings.iter().any(|w| w.contains("backtick")));

        let r = analyze_command_security("echo $(id)");
        assert!(r
            .warnings
            .iter()
            .any(|w| w.contains("command substitution")));

        let r = analyze_command_security("echo ${HOME}");
        assert!(r
            .warnings
            .iter()
            .any(|w| w.contains("parameter substitution")));
    }

    #[test]
    fn test_dangerous_variables() {
        let r = analyze_command_security("cat $FILE | grep foo");
        assert!(r.warnings.iter().any(|w| w.contains("variables")));
    }

    #[test]
    fn test_proc_environ_blocked() {
        let r = analyze_command_security("cat /proc/self/environ");
        assert!(r.blocked.is_some());
        assert!(r.blocked.unwrap().contains("environ"));
    }

    #[test]
    fn test_ifs_injection() {
        let r = analyze_command_security("IFS=/ echo test");
        assert!(r.warnings.iter().any(|w| w.contains("IFS")));
    }

    #[test]
    fn test_control_characters_blocked() {
        let r = analyze_command_security("echo \x07hello");
        assert!(r.blocked.is_some());
    }

    #[test]
    fn test_zsh_dangerous_commands() {
        let r = analyze_command_security("zmodload zsh/system");
        assert!(r.blocked.is_some());

        let r = analyze_command_security("zpty test bash");
        assert!(r.blocked.is_some());
    }

    #[test]
    fn test_safe_commands_pass() {
        // These should have no blocked and no warnings
        for cmd in &[
            "echo hello",
            "pwd",
            "ls",
            "cat file.txt",
            "grep pattern file",
        ] {
            let r = analyze_command_security(cmd);
            assert!(
                r.blocked.is_none(),
                "'{cmd}' should not be blocked: {:?}",
                r.blocked
            );
        }
    }

    #[test]
    fn test_truncate_output() {
        let short = "hello\nworld\n";
        let (out, trunc) = truncate_output(short, 1000);
        assert_eq!(out, short);
        assert!(!trunc);

        let long = "a\n".repeat(1000);
        let (out, trunc) = truncate_output(&long, 100);
        assert!(trunc);
        assert!(out.len() < long.len());
        assert!(out.contains("truncated"));
    }

    #[test]
    fn test_read_only_commands() {
        assert!(is_read_only_command("ls -la"));
        assert!(is_read_only_command("pwd"));
        assert!(is_read_only_command("cat file.txt"));
        assert!(is_read_only_command("git status"));
        assert!(is_read_only_command("git log --oneline"));
        assert!(is_read_only_command("rg pattern"));
        assert!(is_read_only_command("echo hello | wc -l"));
        assert!(is_read_only_command("find . -name '*.rs'"));

        assert!(!is_read_only_command("rm file.txt"));
        assert!(!is_read_only_command("git push"));
        assert!(!is_read_only_command("npm install"));
        assert!(!is_read_only_command("cargo build"));
        assert!(!is_read_only_command("mkdir -p dir"));
    }

    #[test]
    fn test_bash_permission_read_only_auto_allow() {
        use rclaude_core::permissions::PermissionMode;
        let result = check_bash_permission(
            "ls -la",
            PermissionMode::Default,
            &[],
            std::path::Path::new("/tmp"),
        );
        assert!(matches!(result, BashPermissionResult::Allow));
    }

    #[test]
    fn test_bash_permission_blocked_command() {
        use rclaude_core::permissions::PermissionMode;
        let result = check_bash_permission(
            "rm -rf /",
            PermissionMode::Auto,
            &[],
            std::path::Path::new("/tmp"),
        );
        assert!(matches!(result, BashPermissionResult::Deny(_)));
    }

    #[test]
    fn test_bash_permission_bypass_mode() {
        use rclaude_core::permissions::PermissionMode;
        let result = check_bash_permission(
            "npm install",
            PermissionMode::BypassPermissions,
            &[],
            std::path::Path::new("/tmp"),
        );
        assert!(matches!(result, BashPermissionResult::Allow));
    }

    #[test]
    fn test_bash_permission_plan_mode_blocks_writes() {
        use rclaude_core::permissions::PermissionMode;
        let result = check_bash_permission(
            "npm install",
            PermissionMode::Plan,
            &[],
            std::path::Path::new("/tmp"),
        );
        assert!(matches!(result, BashPermissionResult::Deny(_)));
    }

    #[test]
    fn test_bash_permission_explicit_allow_rule() {
        use rclaude_core::permissions::*;
        let rules = vec![PermissionRule {
            source: PermissionRuleSource::ProjectSettings,
            behavior: PermissionBehavior::Allow,
            tool_name: "Bash".into(),
            rule_content: Some("npm install".into()),
        }];
        let result = check_bash_permission(
            "npm install",
            PermissionMode::Default,
            &rules,
            std::path::Path::new("/tmp"),
        );
        assert!(matches!(result, BashPermissionResult::Allow));
    }

    #[test]
    fn test_bash_permission_auto_mode_safe_command() {
        use rclaude_core::permissions::PermissionMode;
        // Simple command with no warnings, AST parses OK → auto-allow
        let result = check_bash_permission(
            "echo hello",
            PermissionMode::Auto,
            &[],
            std::path::Path::new("/tmp"),
        );
        assert!(matches!(result, BashPermissionResult::Allow));
    }
}
