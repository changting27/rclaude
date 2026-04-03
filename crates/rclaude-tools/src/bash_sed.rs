//! Sed command validation.
//!
//! Two safe patterns:
//! 1. Line printing: `sed -n 'Np'` / `sed -n 'N,Mp'`
//! 2. Substitution: `sed 's/pattern/replacement/flags'` (flags: g,p,i,I,m,M,1-9)
//!
//! Everything else → requires user approval.

use regex::Regex;

/// Check if a sed command is allowed by the allowlist.
pub fn sed_command_is_allowed(command: &str, allow_file_writes: bool) -> bool {
    let expressions = match extract_sed_expressions(command) {
        Ok(e) => e,
        Err(_) => return false,
    };

    let has_files = has_file_args(command);

    let is_pattern1 = is_line_printing_command(command, &expressions);
    let is_pattern2 = is_substitution_command(command, &expressions, has_files, allow_file_writes);

    if !is_pattern1 && !is_pattern2 {
        return false;
    }

    // Pattern 2 does not allow semicolons
    if is_pattern2 {
        for expr in &expressions {
            if expr.contains(';') {
                return false;
            }
        }
    }

    // Defense-in-depth: denylist check
    for expr in &expressions {
        if contains_dangerous_operations(expr) {
            return false;
        }
    }

    true
}

/// Pattern 1: Line printing with -n flag.
/// Allows: sed -n 'Np', sed -n 'N,Mp', sed -n '1p;2p;3p'
fn is_line_printing_command(command: &str, expressions: &[String]) -> bool {
    let args = match shell_words::split(command) {
        Ok(a) => a,
        Err(_) => return false,
    };
    if args.first().is_none_or(|a| a != "sed") {
        return false;
    }

    let flags: Vec<&str> = args[1..]
        .iter()
        .filter(|a| a.starts_with('-') && *a != "--")
        .map(|a| a.as_str())
        .collect();

    let allowed = [
        "-n",
        "--quiet",
        "--silent",
        "-E",
        "--regexp-extended",
        "-r",
        "-z",
        "--zero-terminated",
        "--posix",
    ];
    if !validate_flags(&flags, &allowed) {
        return false;
    }

    // Must have -n
    let has_n = flags.iter().any(|f| {
        *f == "-n"
            || *f == "--quiet"
            || *f == "--silent"
            || (f.starts_with('-') && !f.starts_with("--") && f.contains('n'))
    });
    if !has_n {
        return false;
    }

    if expressions.is_empty() {
        return false;
    }

    // All expressions must be print commands
    for expr in expressions {
        for cmd in expr.split(';') {
            if !is_print_command(cmd.trim()) {
                return false;
            }
        }
    }
    true
}

/// Check if a single command is a valid print command: p, Np, N,Mp
fn is_print_command(cmd: &str) -> bool {
    if cmd.is_empty() {
        return false;
    }
    Regex::new(r"^(?:\d+|\d+,\d+)?p$").unwrap().is_match(cmd)
}

/// Pattern 2: Substitution command.
fn is_substitution_command(
    command: &str,
    expressions: &[String],
    has_files: bool,
    allow_file_writes: bool,
) -> bool {
    if !allow_file_writes && has_files {
        return false;
    }

    let args = match shell_words::split(command) {
        Ok(a) => a,
        Err(_) => return false,
    };
    if args.first().is_none_or(|a| a != "sed") {
        return false;
    }

    let flags: Vec<&str> = args[1..]
        .iter()
        .filter(|a| a.starts_with('-') && *a != "--")
        .map(|a| a.as_str())
        .collect();

    let mut allowed = vec!["-E", "--regexp-extended", "-r", "--posix"];
    if allow_file_writes {
        allowed.extend_from_slice(&["-i", "--in-place"]);
    }
    if !validate_flags(&flags, &allowed) {
        return false;
    }

    if expressions.len() != 1 {
        return false;
    }
    let expr = expressions[0].trim();

    if !expr.starts_with('s') {
        return false;
    }

    // Parse s/pattern/replacement/flags — only / as delimiter
    let re = Regex::new(r"^s/").unwrap();
    if !re.is_match(expr) {
        return false;
    }

    let rest = &expr[2..]; // after "s/"
    let mut delim_count = 0;
    let mut last_delim = 0;
    let mut i = 0;
    let bytes = rest.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        if bytes[i] == b'/' {
            delim_count += 1;
            last_delim = i;
        }
        i += 1;
    }
    if delim_count != 2 {
        return false;
    }

    // Validate flags after last delimiter
    let expr_flags = &rest[last_delim + 1..];
    let flag_re = Regex::new(r"^[gpimIM]*[1-9]?[gpimIM]*$").unwrap();
    if !flag_re.is_match(expr_flags) {
        return false;
    }

    true
}

/// Validate flags against an allowlist (handles combined flags like -nE).
fn validate_flags(flags: &[&str], allowed: &[&str]) -> bool {
    for flag in flags {
        if flag.starts_with("--") {
            if !allowed.contains(flag) {
                return false;
            }
        } else if flag.len() > 2 {
            // Combined flags like -nE
            for c in flag[1..].chars() {
                let single = format!("-{c}");
                if !allowed.contains(&single.as_str()) {
                    return false;
                }
            }
        } else if !allowed.contains(flag) {
            return false;
        }
    }
    true
}

/// Check if sed command has file arguments.
fn has_file_args(command: &str) -> bool {
    let args = match shell_words::split(command) {
        Ok(a) => a,
        Err(_) => return true,
    };
    if args.first().is_none_or(|a| a != "sed") {
        return true;
    }

    let mut arg_count = 0;
    let mut has_e = false;
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if (arg == "-e" || arg == "--expression") && i + 1 < args.len() {
            has_e = true;
            i += 2;
            continue;
        }
        if arg.starts_with("--expression=") || arg.starts_with("-e=") {
            has_e = true;
            i += 1;
            continue;
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        arg_count += 1;
        if has_e {
            return true;
        }
        if arg_count > 1 {
            return true;
        }
        i += 1;
    }
    false
}

/// Extract sed expressions from command.
fn extract_sed_expressions(command: &str) -> Result<Vec<String>, String> {
    let args = shell_words::split(command).map_err(|e| e.to_string())?;
    if args.first().is_none_or(|a| a != "sed") {
        return Ok(vec![]);
    }

    // Reject dangerous flag combinations
    let joined = args[1..].join(" ");
    if Regex::new(r"-e[wWe]|-w[eE]").unwrap().is_match(&joined) {
        return Err("Dangerous flag combination".into());
    }

    let mut expressions = Vec::new();
    let mut found_e = false;
    let mut found_expr = false;
    let mut i = 1;

    while i < args.len() {
        let arg = &args[i];
        if (arg == "-e" || arg == "--expression") && i + 1 < args.len() {
            found_e = true;
            expressions.push(args[i + 1].clone());
            i += 2;
            continue;
        }
        if let Some(val) = arg.strip_prefix("--expression=") {
            found_e = true;
            expressions.push(val.to_string());
            i += 1;
            continue;
        }
        if let Some(val) = arg.strip_prefix("-e=") {
            found_e = true;
            expressions.push(val.to_string());
            i += 1;
            continue;
        }
        if arg.starts_with('-') {
            i += 1;
            continue;
        }
        if !found_e && !found_expr {
            expressions.push(arg.clone());
            found_expr = true;
            i += 1;
            continue;
        }
        break;
    }
    Ok(expressions)
}

/// Check if a sed expression contains dangerous operations.
fn contains_dangerous_operations(expression: &str) -> bool {
    let cmd = expression.trim();
    if cmd.is_empty() {
        return false;
    }

    // Non-ASCII characters
    if cmd.bytes().any(|b| b > 0x7F) {
        return true;
    }
    // Curly braces (blocks)
    if cmd.contains('{') || cmd.contains('}') {
        return true;
    }
    // Newlines
    if cmd.contains('\n') {
        return true;
    }
    // Comments (# not after s)
    if let Some(pos) = cmd.find('#') {
        if pos == 0 || cmd.as_bytes().get(pos.wrapping_sub(1)) != Some(&b's') {
            return true;
        }
    }
    // Negation
    if cmd.starts_with('!') {
        return true;
    }
    if Regex::new(r"[/\d$]!").unwrap().is_match(cmd) {
        return true;
    }
    // Tilde step address
    if Regex::new(r"\d\s*~\s*\d|,\s*~\s*\d|\$\s*~\s*\d")
        .unwrap()
        .is_match(cmd)
    {
        return true;
    }
    // Comma at start
    if cmd.starts_with(',') {
        return true;
    }
    // Comma + offset
    if Regex::new(r",\s*[+-]").unwrap().is_match(cmd) {
        return true;
    }
    // Backslash tricks
    if Regex::new(r"s\\|\\[|#%@]").unwrap().is_match(cmd) {
        return true;
    }

    // Write commands (w/W)
    if Regex::new(r"^[wW]\s*\S+|^\d+\s*[wW]\s*\S+|^\$\s*[wW]\s*\S+")
        .unwrap()
        .is_match(cmd)
    {
        return true;
    }
    if Regex::new(r"^/[^/]*/[IMim]*\s*[wW]\s*\S+")
        .unwrap()
        .is_match(cmd)
    {
        return true;
    }

    // Execute commands (e)
    if cmd.starts_with('e') {
        return true;
    }
    if Regex::new(r"^\d+\s*e|^\$\s*e|^/[^/]*/[IMim]*\s*e")
        .unwrap()
        .is_match(cmd)
    {
        return true;
    }

    // Substitution with dangerous flags (w/W/e/E)
    // Parse s<delim>...<delim>...<delim><flags> manually since regex crate lacks backrefs
    if cmd.starts_with('s') && cmd.len() > 1 {
        let delim = cmd.as_bytes()[1];
        if delim != b'\\' && delim != b'\n' {
            // Find 3rd delimiter
            let mut count = 0;
            let mut last_pos = 0;
            let mut i = 2;
            let bytes = cmd.as_bytes();
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == delim {
                    count += 1;
                    last_pos = i;
                }
                i += 1;
            }
            if count >= 2 {
                let flags = &cmd[last_pos + 1..];
                if flags.contains('w')
                    || flags.contains('W')
                    || flags.contains('e')
                    || flags.contains('E')
                {
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_print_commands() {
        assert!(sed_command_is_allowed("sed -n '5p'", false));
        assert!(sed_command_is_allowed("sed -n '1,10p'", false));
        assert!(sed_command_is_allowed("sed -n '1p;2p;3p'", false));
    }

    #[test]
    fn test_safe_substitution() {
        assert!(sed_command_is_allowed("sed 's/foo/bar/'", false));
        assert!(sed_command_is_allowed("sed 's/foo/bar/g'", false));
        assert!(sed_command_is_allowed("sed -E 's/foo/bar/gi'", false));
    }

    #[test]
    fn test_dangerous_write() {
        assert!(!sed_command_is_allowed(
            "sed 's/foo/bar/w output.txt'",
            false
        ));
        assert!(!sed_command_is_allowed("sed '1w output.txt'", false));
    }

    #[test]
    fn test_dangerous_execute() {
        assert!(!sed_command_is_allowed("sed 's/foo/bar/e'", false));
        assert!(!sed_command_is_allowed("sed 'e date'", false));
    }

    #[test]
    fn test_inplace_requires_allow() {
        assert!(!sed_command_is_allowed(
            "sed -i 's/foo/bar/' file.txt",
            false
        ));
        assert!(sed_command_is_allowed("sed -i 's/foo/bar/' file.txt", true));
    }

    #[test]
    fn test_file_args_detection() {
        assert!(!has_file_args("sed 's/foo/bar/'"));
        assert!(has_file_args("sed 's/foo/bar/' file.txt"));
        assert!(has_file_args("sed -e 's/foo/bar/' file.txt"));
    }

    #[test]
    fn test_is_print_command() {
        assert!(is_print_command("p"));
        assert!(is_print_command("5p"));
        assert!(is_print_command("1,10p"));
        assert!(!is_print_command("d"));
        assert!(!is_print_command("w file"));
    }

    #[test]
    fn test_dangerous_operations() {
        assert!(contains_dangerous_operations("e date"));
        assert!(contains_dangerous_operations("w output.txt"));
        assert!(contains_dangerous_operations("{d}"));
        assert!(!contains_dangerous_operations("s/foo/bar/g"));
        assert!(!contains_dangerous_operations("5p"));
    }

    #[test]
    fn test_extract_expressions() {
        let exprs = extract_sed_expressions("sed 's/foo/bar/'").unwrap();
        assert_eq!(exprs, vec!["s/foo/bar/"]);

        let exprs = extract_sed_expressions("sed -e 's/a/b/' -e 's/c/d/'").unwrap();
        assert_eq!(exprs.len(), 2);
    }
}
