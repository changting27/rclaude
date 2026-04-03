//! Bash command AST analysis using tree-sitter-bash.
//!
//! Fail-closed design:
//! - Parse with tree-sitter-bash, walk the AST with an EXPLICIT allowlist
//! - Any unknown node type → "too-complex" → ask user
//! - Extract argv[], env vars, redirects for each simple command
//! - Track variable scope across && / ; chains
//! - Recursively extract $() inner commands

use regex::Regex;
use std::collections::HashMap;

use tree_sitter::{Language, Node, Parser};

// ── Types ──

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redirect {
    pub op: RedirectOp,
    pub target: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectOp {
    Write,
    Append,
    Read,
    HereDoc,
    HereString,
    DupOut,
    DupIn,
    Clobber,
}

#[derive(Debug, Clone)]
pub struct SimpleCommand {
    pub argv: Vec<String>,
    pub env_vars: Vec<(String, String)>,
    pub redirects: Vec<Redirect>,
    pub text: String,
}

#[derive(Debug, Clone)]
pub enum ParseResult {
    Simple(Vec<SimpleCommand>),
    TooComplex { reason: String },
    ParseUnavailable,
}

// ── Constants ──

const MAX_COMMAND_LENGTH: usize = 10_000;
const CMDSUB_PLACEHOLDER: &str = "__CMDSUB_OUTPUT__";
const VAR_PLACEHOLDER: &str = "__TRACKED_VAR__";

const SAFE_ENV_VARS: &[&str] = &[
    "HOME",
    "PWD",
    "OLDPWD",
    "USER",
    "LOGNAME",
    "SHELL",
    "PATH",
    "HOSTNAME",
    "UID",
    "EUID",
    "PPID",
    "RANDOM",
    "SECONDS",
    "LINENO",
    "TMPDIR",
    "BASH_VERSION",
    "BASHPID",
    "SHLVL",
];

const EVAL_LIKE_BUILTINS: &[&str] = &["eval", "source", ".", "exec", "trap", "enable"];

const STRUCTURAL_TYPES: &[&str] = &["program", "list", "pipeline", "redirected_statement"];
const SEPARATOR_TYPES: &[&str] = &["&&", "||", "|", ";", "&", "|&", "\n"];

lazy_static::lazy_static! {
    static ref CONTROL_CHAR_RE: Regex = Regex::new(r"[\x00-\x08\x0b-\x1f\x7f]").unwrap();
    static ref UNICODE_WS_RE: Regex = Regex::new(r"[\u{00a0}\u{1680}\u{2000}-\u{200b}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}]").unwrap();
    static ref BACKSLASH_WS_RE: Regex = Regex::new(r"\\[ \t]|[^ \t\n\\]\\\n").unwrap();
    static ref BRACE_EXPANSION_RE: Regex = Regex::new(r"\{[^{}\s]*(,|\.\.)[^{}\s]*\}").unwrap();
    static ref ZSH_TILDE_BRACKET_RE: Regex = Regex::new(r"~\[").unwrap();
    static ref ZSH_EQUALS_RE: Regex = Regex::new(r"(?:^|[\s;&|])=[a-zA-Z_]").unwrap();
    static ref BARE_VAR_UNSAFE_RE: Regex = Regex::new(r"[ \t\n*?\[]").unwrap();
}

// ── Parser initialization ──

fn get_language() -> Language {
    tree_sitter_bash::LANGUAGE.into()
}

fn create_parser() -> Option<Parser> {
    let mut parser = Parser::new();
    parser.set_language(&get_language()).ok()?;
    Some(parser)
}

// ── Main entry point ──

/// Parse a bash command for security analysis using tree-sitter.
pub fn parse_for_security(cmd: &str) -> ParseResult {
    // Pre-checks
    if cmd.is_empty() {
        return ParseResult::Simple(vec![]);
    }
    if cmd.len() > MAX_COMMAND_LENGTH {
        return ParseResult::TooComplex {
            reason: "Command too long".into(),
        };
    }
    if CONTROL_CHAR_RE.is_match(cmd) {
        return ParseResult::TooComplex {
            reason: "Contains control characters".into(),
        };
    }
    if UNICODE_WS_RE.is_match(cmd) {
        return ParseResult::TooComplex {
            reason: "Contains Unicode whitespace".into(),
        };
    }
    if cmd.contains('\r') {
        return ParseResult::TooComplex {
            reason: "Contains carriage return".into(),
        };
    }
    if BACKSLASH_WS_RE.is_match(cmd) {
        return ParseResult::TooComplex {
            reason: "Contains backslash-escaped whitespace".into(),
        };
    }
    if ZSH_TILDE_BRACKET_RE.is_match(cmd) {
        return ParseResult::TooComplex {
            reason: "Contains zsh ~[ syntax".into(),
        };
    }
    if ZSH_EQUALS_RE.is_match(cmd) {
        return ParseResult::TooComplex {
            reason: "Contains zsh =cmd expansion".into(),
        };
    }

    // Parse with tree-sitter
    let mut parser = match create_parser() {
        Some(p) => p,
        None => return ParseResult::ParseUnavailable,
    };

    let tree = match parser.parse(cmd, None) {
        Some(t) => t,
        None => {
            return ParseResult::TooComplex {
                reason: "Parse failed".into(),
            }
        }
    };

    let root = tree.root_node();

    // Check for ERROR nodes at root level
    if root.has_error() {
        // Walk to find the specific error
        if has_error_node(root) {
            return ParseResult::TooComplex {
                reason: "Parse error in command".into(),
            };
        }
    }

    // Walk the AST
    let mut commands = Vec::new();
    let mut var_scope = HashMap::new();

    match collect_commands(root, &mut commands, &mut var_scope, cmd) {
        Some(err) => err,
        None => ParseResult::Simple(commands),
    }
}

fn has_error_node(node: Node) -> bool {
    if node.is_error() || node.is_missing() {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_error() || child.is_missing() {
            return true;
        }
    }
    false
}

// ── AST Walking ──

/// Recursively collect simple commands from structural nodes.
fn collect_commands<'a>(
    node: Node<'a>,
    commands: &mut Vec<SimpleCommand>,
    var_scope: &mut HashMap<String, String>,
    src: &str,
) -> Option<ParseResult> {
    let kind = node.kind();

    if kind == "command" {
        return walk_command(node, commands, var_scope, src, &[]);
    }

    if kind == "redirected_statement" {
        return walk_redirected_statement(node, commands, var_scope, src);
    }

    if kind == "comment" {
        return None; // skip
    }

    if kind == "variable_assignment" {
        // Standalone assignment (not inside a command): DIR=/tmp
        match walk_variable_assignment(node, src, commands, var_scope) {
            Ok(_) => return None,
            Err(e) => return Some(e),
        }
    }

    if kind == "negated_command" {
        // `! cmd` — just recurse into the wrapped command
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "!" {
                continue;
            }
            return collect_commands(child, commands, var_scope, src);
        }
        return None;
    }

    if kind == "declaration_command" {
        // export/local/readonly/declare — extract as a command
        return walk_declaration(node, commands, var_scope, src);
    }

    if STRUCTURAL_TYPES.contains(&kind) {
        // program, list, pipeline — recurse into children
        let is_pipeline = kind == "pipeline";
        let snapshot = if !is_pipeline {
            Some(var_scope.clone())
        } else {
            None
        };
        let mut scope = var_scope.clone();

        // For non-pipeline, use the actual var_scope for && / ; chains
        let scope_ref = if is_pipeline { &mut scope } else { var_scope };

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            let ck = child.kind();
            if SEPARATOR_TYPES.contains(&ck) {
                // After || or | or &, reset scope (vars don't carry)
                if ck == "||" || ck == "|" || ck == "|&" || ck == "&" {
                    if let Some(ref snap) = snapshot {
                        *scope_ref = snap.clone();
                    }
                }
                continue;
            }
            if let Some(err) = collect_commands(child, commands, scope_ref, src) {
                return Some(err);
            }
        }
        return None;
    }

    // Unknown structural node → too complex
    Some(too_complex(
        kind,
        node.utf8_text(src.as_bytes()).unwrap_or(""),
    ))
}

/// Walk a `redirected_statement` node: extract redirects, then recurse into the command.
fn walk_redirected_statement<'a>(
    node: Node<'a>,
    commands: &mut Vec<SimpleCommand>,
    var_scope: &mut HashMap<String, String>,
    src: &str,
) -> Option<ParseResult> {
    let mut extra_redirects = Vec::new();
    let mut command_node = None;
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "file_redirect" => match walk_file_redirect(child, src, commands, var_scope) {
                Ok(r) => extra_redirects.push(r),
                Err(e) => return Some(e),
            },
            "heredoc_redirect" | "herestring_redirect" => {
                return Some(ParseResult::TooComplex {
                    reason: format!("Contains {}", child.kind()),
                });
            }
            _ => {
                command_node = Some(child);
            }
        }
    }

    if let Some(cmd_node) = command_node {
        if cmd_node.kind() == "command" {
            return walk_command(cmd_node, commands, var_scope, src, &extra_redirects);
        }
        // Could be a pipeline or other structural node inside redirected_statement
        // Collect commands normally, then attach redirects to the last command
        let _before = commands.len();
        let result = collect_commands(cmd_node, commands, var_scope, src);
        if result.is_some() {
            return result;
        }
        // Attach redirects to the last command added
        if !extra_redirects.is_empty() {
            if let Some(last) = commands.last_mut() {
                last.redirects.extend(extra_redirects);
            }
        }
        return None;
    }

    None
}

/// Walk a `command` node: extract argv, env vars, redirects.
fn walk_command<'a>(
    node: Node<'a>,
    inner_commands: &mut Vec<SimpleCommand>,
    var_scope: &mut HashMap<String, String>,
    src: &str,
    extra_redirects: &[Redirect],
) -> Option<ParseResult> {
    let mut argv = Vec::new();
    let mut env_vars = Vec::new();
    let mut redirects: Vec<Redirect> = extra_redirects.to_vec();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "variable_assignment" => {
                match walk_variable_assignment(child, src, inner_commands, var_scope) {
                    Ok((name, value)) => {
                        env_vars.push((name, value));
                    }
                    Err(e) => return Some(e),
                }
            }
            "command_name" => {
                let name_child = child.child(0).unwrap_or(child);
                match walk_argument(name_child, src, inner_commands, var_scope) {
                    Ok(s) => argv.push(s),
                    Err(e) => return Some(e),
                }
            }
            "word" | "number" | "raw_string" | "string" | "concatenation" => {
                match walk_argument(child, src, inner_commands, var_scope) {
                    Ok(s) => argv.push(s),
                    Err(e) => return Some(e),
                }
            }
            "simple_expansion" => match resolve_simple_expansion(child, src, var_scope, false) {
                Ok(s) => argv.push(s),
                Err(e) => return Some(e),
            },
            "file_redirect" => match walk_file_redirect(child, src, inner_commands, var_scope) {
                Ok(r) => redirects.push(r),
                Err(e) => return Some(e),
            },
            "herestring_redirect" | "heredoc_redirect" => {
                return Some(ParseResult::TooComplex {
                    reason: format!("Contains {}", child.kind()),
                });
            }
            // Command substitution as bare argument → too complex
            // (output IS the argument, can't trust placeholder for paths)
            "command_substitution" => {
                return Some(ParseResult::TooComplex {
                    reason: "Bare command substitution as argument".into(),
                });
            }
            _ => {
                return Some(too_complex(
                    child.kind(),
                    child.utf8_text(src.as_bytes()).unwrap_or(""),
                ));
            }
        }
    }

    // Check for eval-like builtins
    if let Some(cmd_name) = argv.first() {
        let unescaped = cmd_name.replace('\\', "");
        if EVAL_LIKE_BUILTINS.contains(&unescaped.as_str()) {
            return Some(ParseResult::TooComplex {
                reason: format!("Eval-like builtin: {cmd_name}"),
            });
        }
    }

    let text = node.utf8_text(src.as_bytes()).unwrap_or("").to_string();
    inner_commands.push(SimpleCommand {
        argv,
        env_vars,
        redirects,
        text,
    });
    None
}

/// Walk a declaration_command (export/local/readonly/declare).
fn walk_declaration<'a>(
    node: Node<'a>,
    commands: &mut Vec<SimpleCommand>,
    var_scope: &mut HashMap<String, String>,
    src: &str,
) -> Option<ParseResult> {
    let mut argv = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "export" | "local" | "readonly" | "declare" | "typeset" | "unset" => {
                argv.push(child.utf8_text(src.as_bytes()).unwrap_or("").to_string());
            }
            "variable_assignment" => {
                match walk_variable_assignment(child, src, commands, var_scope) {
                    Ok((name, value)) => {
                        argv.push(format!("{name}={value}"));
                    }
                    Err(e) => return Some(e),
                }
            }
            "word" | "number" | "raw_string" | "string" => {
                match walk_argument(child, src, commands, var_scope) {
                    Ok(s) => argv.push(s),
                    Err(e) => return Some(e),
                }
            }
            "simple_expansion" => match resolve_simple_expansion(child, src, var_scope, false) {
                Ok(s) => argv.push(s),
                Err(e) => return Some(e),
            },
            _ => {
                return Some(too_complex(
                    child.kind(),
                    child.utf8_text(src.as_bytes()).unwrap_or(""),
                ));
            }
        }
    }

    let text = node.utf8_text(src.as_bytes()).unwrap_or("").to_string();
    commands.push(SimpleCommand {
        argv,
        env_vars: vec![],
        redirects: vec![],
        text,
    });
    None
}

// ── Argument resolution ──

/// Walk an argument node to its literal string value.
fn walk_argument<'a>(
    node: Node<'a>,
    src: &str,
    inner_commands: &mut Vec<SimpleCommand>,
    var_scope: &mut HashMap<String, String>,
) -> Result<String, ParseResult> {
    match node.kind() {
        "word" => {
            let text = node.utf8_text(src.as_bytes()).unwrap_or("");
            if BRACE_EXPANSION_RE.is_match(text) {
                return Err(ParseResult::TooComplex {
                    reason: "Brace expansion in word".into(),
                });
            }
            // Unescape backslash sequences (bash quote removal)
            Ok(unescape_word(text))
        }
        "number" => {
            if node.child_count() > 0 {
                return Err(ParseResult::TooComplex {
                    reason: "Number with expansion (NN# base syntax)".into(),
                });
            }
            Ok(node.utf8_text(src.as_bytes()).unwrap_or("").to_string())
        }
        "raw_string" => {
            // Single-quoted string: strip surrounding quotes
            let text = node.utf8_text(src.as_bytes()).unwrap_or("");
            Ok(strip_raw_string(text))
        }
        "string" => {
            // Double-quoted string: resolve escapes and expansions
            walk_string(node, src, inner_commands, var_scope)
        }
        "concatenation" => {
            let text = node.utf8_text(src.as_bytes()).unwrap_or("");
            if BRACE_EXPANSION_RE.is_match(text) {
                return Err(ParseResult::TooComplex {
                    reason: "Brace expansion".into(),
                });
            }
            let mut result = String::new();
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                let part = walk_argument(child, src, inner_commands, var_scope)?;
                result.push_str(&part);
            }
            Ok(result)
        }
        "simple_expansion" => resolve_simple_expansion(node, src, var_scope, false),
        "command_substitution" => Err(ParseResult::TooComplex {
            reason: "Command substitution in argument position".into(),
        }),
        "expansion" => Err(ParseResult::TooComplex {
            reason: "Parameter expansion ${}".into(),
        }),
        "process_substitution" => Err(ParseResult::TooComplex {
            reason: "Process substitution".into(),
        }),
        other => Err(too_complex(
            other,
            node.utf8_text(src.as_bytes()).unwrap_or(""),
        )),
    }
}

/// Walk a double-quoted string node.
fn walk_string<'a>(
    node: Node<'a>,
    src: &str,
    inner_commands: &mut Vec<SimpleCommand>,
    var_scope: &mut HashMap<String, String>,
) -> Result<String, ParseResult> {
    let mut result = String::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "\"" => {} // delimiter
            "string_content" => {
                let text = child.utf8_text(src.as_bytes()).unwrap_or("");
                // Resolve double-quote escapes: only $ ` " \ are special
                result.push_str(
                    &text
                        .replace("\\$", "$")
                        .replace("\\`", "`")
                        .replace("\\\"", "\"")
                        .replace("\\\\", "\\"),
                );
            }
            "$" => {
                result.push('$'); // bare $ before closing quote
            }
            "simple_expansion" => match resolve_simple_expansion(child, src, var_scope, true) {
                Ok(s) => result.push_str(&s),
                Err(e) => return Err(e),
            },
            "command_substitution" => {
                // $() inside "..." — extract inner commands, use placeholder
                let mut inner_scope = var_scope.clone();
                let mut sub_cursor = child.walk();
                for sub_child in child.children(&mut sub_cursor) {
                    let sk = sub_child.kind();
                    if sk == "$(" || sk == "`" || sk == ")" {
                        continue;
                    }
                    if let Some(err) =
                        collect_commands(sub_child, inner_commands, &mut inner_scope, src)
                    {
                        return Err(err);
                    }
                }
                result.push_str(CMDSUB_PLACEHOLDER);
            }
            "expansion" => {
                return Err(ParseResult::TooComplex {
                    reason: "Parameter expansion ${} in string".into(),
                });
            }
            other => {
                return Err(too_complex(
                    other,
                    child.utf8_text(src.as_bytes()).unwrap_or(""),
                ));
            }
        }
    }
    Ok(result)
}

/// Resolve a simple_expansion ($VAR) node.
fn resolve_simple_expansion(
    node: Node,
    src: &str,
    var_scope: &HashMap<String, String>,
    inside_string: bool,
) -> Result<String, ParseResult> {
    let text = node.utf8_text(src.as_bytes()).unwrap_or("");
    let var_name = text.strip_prefix('$').unwrap_or(text);

    // Safe env vars
    if SAFE_ENV_VARS.contains(&var_name) {
        if inside_string {
            return Ok(VAR_PLACEHOLDER.to_string());
        }
        // Bare $HOME etc. — safe as single arg
        return Ok(VAR_PLACEHOLDER.to_string());
    }

    // Tracked variable from earlier assignment
    if let Some(value) = var_scope.get(var_name) {
        if value.contains(CMDSUB_PLACEHOLDER) || value.contains(VAR_PLACEHOLDER) {
            if inside_string {
                return Ok(VAR_PLACEHOLDER.to_string());
            }
            return Err(ParseResult::TooComplex {
                reason: format!("Bare $-expansion of dynamic variable: {var_name}"),
            });
        }
        // Check for IFS/glob chars in bare context
        if !inside_string && BARE_VAR_UNSAFE_RE.is_match(value) {
            return Err(ParseResult::TooComplex {
                reason: format!("Variable ${var_name} contains word-splitting chars"),
            });
        }
        return Ok(value.clone());
    }

    Err(ParseResult::TooComplex {
        reason: format!("Untracked variable: ${var_name}"),
    })
}

/// Walk a variable_assignment node (VAR=value).
fn walk_variable_assignment<'a>(
    node: Node<'a>,
    src: &str,
    inner_commands: &mut Vec<SimpleCommand>,
    var_scope: &mut HashMap<String, String>,
) -> Result<(String, String), ParseResult> {
    let mut name = String::new();
    let mut value = String::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "variable_name" => {
                name = child.utf8_text(src.as_bytes()).unwrap_or("").to_string();
            }
            "=" => {}
            "word" | "number" | "raw_string" | "string" | "concatenation" => {
                value = walk_argument(child, src, inner_commands, var_scope)?;
            }
            "command_substitution" => {
                // VAR=$(cmd) — extract inner command, value is placeholder
                let mut inner_scope = var_scope.clone();
                let mut sub_cursor = child.walk();
                for sub_child in child.children(&mut sub_cursor) {
                    let sk = sub_child.kind();
                    if sk == "$(" || sk == "`" || sk == ")" {
                        continue;
                    }
                    if let Some(err) =
                        collect_commands(sub_child, inner_commands, &mut inner_scope, src)
                    {
                        return Err(err);
                    }
                }
                value = CMDSUB_PLACEHOLDER.to_string();
            }
            "simple_expansion" => {
                value = resolve_simple_expansion(child, src, var_scope, true)?;
            }
            other
                if other.is_empty() || child.utf8_text(src.as_bytes()).unwrap_or("").is_empty() =>
            {
                // Empty value: VAR=
            }
            other => {
                return Err(too_complex(
                    other,
                    child.utf8_text(src.as_bytes()).unwrap_or(""),
                ));
            }
        }
    }

    var_scope.insert(name.clone(), value.clone());
    Ok((name, value))
}

/// Walk a file_redirect node.
fn walk_file_redirect<'a>(
    node: Node<'a>,
    src: &str,
    inner_commands: &mut Vec<SimpleCommand>,
    var_scope: &mut HashMap<String, String>,
) -> Result<Redirect, ParseResult> {
    let mut op = RedirectOp::Write;
    let mut target = String::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            ">" => op = RedirectOp::Write,
            ">>" => op = RedirectOp::Append,
            "<" => op = RedirectOp::Read,
            ">&" => op = RedirectOp::DupOut,
            "<&" => op = RedirectOp::DupIn,
            ">|" => op = RedirectOp::Clobber,
            "file_descriptor" => {} // fd number before redirect
            "word" | "number" | "raw_string" | "string" => {
                target = walk_argument(child, src, inner_commands, var_scope)?;
            }
            "simple_expansion" => {
                // $VAR in redirect target — reject (can't trust path)
                return Err(ParseResult::TooComplex {
                    reason: "Variable expansion in redirect target".into(),
                });
            }
            _ => {}
        }
    }

    Ok(Redirect { op, target })
}

// ── Helpers ──

fn unescape_word(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                result.push(next);
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn strip_raw_string(text: &str) -> String {
    // Remove surrounding single quotes: 'content' → content
    if text.starts_with('\'') && text.ends_with('\'') && text.len() >= 2 {
        text[1..text.len() - 1].to_string()
    } else if text.starts_with("$'") && text.ends_with('\'') && text.len() >= 3 {
        // ANSI-C string $'...' — too complex for now
        text[2..text.len() - 1].to_string()
    } else {
        text.to_string()
    }
}

fn too_complex(node_type: &str, text: &str) -> ParseResult {
    let preview = if text.len() > 40 { &text[..40] } else { text };
    ParseResult::TooComplex {
        reason: format!("Unsupported node type '{node_type}': {preview}"),
    }
}

/// Extract command names from a parse result.
pub fn extract_command_names(result: &ParseResult) -> Vec<String> {
    match result {
        ParseResult::Simple(commands) => commands
            .iter()
            .filter_map(|cmd| cmd.argv.first().cloned())
            .collect(),
        _ => vec![],
    }
}

/// Check if all commands in a parse result are read-only.
pub fn is_parsed_read_only(result: &ParseResult) -> bool {
    match result {
        ParseResult::TooComplex { .. } | ParseResult::ParseUnavailable => false,
        ParseResult::Simple(commands) => commands
            .iter()
            .all(|cmd| crate::bash::is_read_only_command(&cmd.text)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_command() {
        match parse_for_security("ls -la") {
            ParseResult::Simple(cmds) => {
                assert_eq!(cmds.len(), 1);
                assert_eq!(cmds[0].argv, vec!["ls", "-la"]);
            }
            other => panic!("Expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn test_compound_and() {
        match parse_for_security("echo hello && ls") {
            ParseResult::Simple(cmds) => {
                assert_eq!(cmds.len(), 2);
                assert_eq!(cmds[0].argv[0], "echo");
                assert_eq!(cmds[1].argv[0], "ls");
            }
            other => panic!("Expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn test_pipeline() {
        match parse_for_security("cat file | grep pattern | wc -l") {
            ParseResult::Simple(cmds) => {
                assert_eq!(cmds.len(), 3);
                assert_eq!(cmds[0].argv[0], "cat");
                assert_eq!(cmds[1].argv[0], "grep");
                assert_eq!(cmds[2].argv[0], "wc");
            }
            other => panic!("Expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn test_semicolon() {
        match parse_for_security("echo a; echo b; echo c") {
            ParseResult::Simple(cmds) => assert_eq!(cmds.len(), 3),
            other => panic!("Expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn test_env_var_assignment() {
        match parse_for_security("FOO=bar echo test") {
            ParseResult::Simple(cmds) => {
                assert_eq!(cmds[0].env_vars, vec![("FOO".into(), "bar".into())]);
                assert_eq!(cmds[0].argv[0], "echo");
            }
            other => panic!("Expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn test_var_scope_across_and() {
        match parse_for_security("DIR=/tmp && ls $DIR") {
            ParseResult::Simple(cmds) => {
                let ls_cmd = cmds
                    .iter()
                    .find(|c| c.argv.first().map_or(false, |a| a == "ls"))
                    .unwrap();
                assert_eq!(ls_cmd.argv[1], "/tmp");
            }
            other => panic!("Expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn test_command_substitution_in_string() {
        // $() inside "..." should extract inner command
        match parse_for_security("echo \"result: $(git rev-parse HEAD)\"") {
            ParseResult::Simple(cmds) => {
                // Should have both outer echo and inner git
                assert!(
                    cmds.len() >= 2,
                    "Expected >=2 commands, got {}: {:?}",
                    cmds.len(),
                    cmds
                );
                let has_git = cmds
                    .iter()
                    .any(|c| c.argv.first().map_or(false, |a| a == "git"));
                assert!(has_git, "Should extract inner git command");
            }
            other => panic!("Expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn test_bare_command_substitution_rejected() {
        match parse_for_security("rm $(echo /etc)") {
            ParseResult::TooComplex { reason } => {
                assert!(
                    reason.to_lowercase().contains("substitution")
                        || reason.to_lowercase().contains("command"),
                    "reason: {reason}"
                );
            }
            other => panic!("Expected TooComplex, got {:?}", other),
        }
    }

    #[test]
    fn test_eval_rejected() {
        match parse_for_security("eval 'rm -rf /'") {
            ParseResult::TooComplex { reason } => {
                assert!(
                    reason.contains("eval") || reason.contains("Eval"),
                    "reason: {reason}"
                );
            }
            other => panic!("Expected TooComplex, got {:?}", other),
        }
    }

    #[test]
    fn test_control_chars_rejected() {
        match parse_for_security("echo \x07hello") {
            ParseResult::TooComplex { reason } => {
                assert!(reason.contains("control"), "reason: {reason}");
            }
            other => panic!("Expected TooComplex, got {:?}", other),
        }
    }

    #[test]
    fn test_brace_expansion_rejected() {
        match parse_for_security("echo {a,b,c}") {
            ParseResult::TooComplex { reason } => {
                assert!(reason.to_lowercase().contains("brace"), "reason: {reason}");
            }
            other => panic!("Expected TooComplex, got {:?}", other),
        }
    }

    #[test]
    fn test_safe_env_var() {
        match parse_for_security("echo $HOME") {
            ParseResult::Simple(cmds) => {
                assert_eq!(cmds[0].argv[0], "echo");
                assert_eq!(cmds[0].argv[1], VAR_PLACEHOLDER);
            }
            other => panic!("Expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn test_untracked_var_rejected() {
        match parse_for_security("rm $UNKNOWN_VAR") {
            ParseResult::TooComplex { reason } => {
                assert!(
                    reason.contains("ntracked") || reason.contains("variable"),
                    "reason: {reason}"
                );
            }
            other => panic!("Expected TooComplex, got {:?}", other),
        }
    }

    #[test]
    fn test_redirect_extraction() {
        match parse_for_security("echo hello > output.txt") {
            ParseResult::Simple(cmds) => {
                assert_eq!(cmds[0].argv, vec!["echo", "hello"]);
                assert_eq!(cmds[0].redirects.len(), 1);
                assert_eq!(cmds[0].redirects[0].op, RedirectOp::Write);
                assert_eq!(cmds[0].redirects[0].target, "output.txt");
            }
            other => panic!("Expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn test_quoted_string() {
        match parse_for_security("echo 'hello world'") {
            ParseResult::Simple(cmds) => {
                assert_eq!(cmds[0].argv, vec!["echo", "hello world"]);
            }
            other => panic!("Expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn test_double_quoted_string() {
        match parse_for_security("echo \"hello world\"") {
            ParseResult::Simple(cmds) => {
                assert_eq!(cmds[0].argv, vec!["echo", "hello world"]);
            }
            other => panic!("Expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn test_empty_command() {
        match parse_for_security("") {
            ParseResult::Simple(cmds) => assert!(cmds.is_empty()),
            other => panic!("Expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn test_extract_command_names() {
        let result = parse_for_security("git status && npm test");
        let names = extract_command_names(&result);
        assert_eq!(names, vec!["git", "npm"]);
    }

    #[test]
    fn test_long_command_rejected() {
        let long = "echo ".to_string() + &"a".repeat(10001);
        match parse_for_security(&long) {
            ParseResult::TooComplex { reason } => {
                assert!(reason.contains("long"), "reason: {reason}");
            }
            other => panic!("Expected TooComplex, got {:?}", other),
        }
    }

    #[test]
    fn test_negated_command() {
        match parse_for_security("! grep error log.txt") {
            ParseResult::Simple(cmds) => {
                assert_eq!(cmds[0].argv[0], "grep");
            }
            other => panic!("Expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn test_export_declaration() {
        match parse_for_security("export PATH=/usr/bin") {
            ParseResult::Simple(cmds) => {
                assert!(!cmds.is_empty());
                assert_eq!(cmds[0].argv[0], "export");
            }
            other => panic!("Expected Simple, got {:?}", other),
        }
    }

    #[test]
    fn test_var_in_command_substitution_assignment() {
        // SHA=$(cmd) sets SHA to placeholder. Bare $SHA is too-complex
        // (dynamic var as bare arg is rejected).
        // But the inner git command should still be extracted.
        match parse_for_security("SHA=$(git rev-parse HEAD)") {
            ParseResult::Simple(cmds) => {
                let has_git = cmds
                    .iter()
                    .any(|c| c.argv.first().map_or(false, |a| a == "git"));
                assert!(has_git, "Should extract git from $(): {:?}", cmds);
            }
            other => panic!("Expected Simple for assignment, got {:?}", other),
        }
        // Using $SHA as bare arg → too-complex (correct behavior)
        match parse_for_security("SHA=$(git rev-parse HEAD) && echo $SHA") {
            ParseResult::TooComplex { .. } => {} // expected: bare dynamic var
            other => panic!("Expected TooComplex for bare dynamic var, got {:?}", other),
        }
        // Using "$SHA" inside string → OK (placeholder is safe in string context)
        match parse_for_security("SHA=$(git rev-parse HEAD) && echo \"sha: $SHA\"") {
            ParseResult::Simple(cmds) => {
                assert!(cmds.len() >= 2, "Should have git + echo: {:?}", cmds);
            }
            other => panic!("Expected Simple for string context, got {:?}", other),
        }
    }

    #[test]
    fn test_if_statement_rejected() {
        match parse_for_security("if true; then echo hi; fi") {
            ParseResult::TooComplex { .. } => {} // expected
            other => panic!("Expected TooComplex, got {:?}", other),
        }
    }

    #[test]
    fn test_for_loop_rejected() {
        match parse_for_security("for i in 1 2 3; do echo $i; done") {
            ParseResult::TooComplex { .. } => {} // expected
            other => panic!("Expected TooComplex, got {:?}", other),
        }
    }

    #[test]
    fn test_process_substitution_rejected() {
        match parse_for_security("diff <(ls dir1) <(ls dir2)") {
            ParseResult::TooComplex { .. } => {} // expected
            other => panic!("Expected TooComplex, got {:?}", other),
        }
    }
}
