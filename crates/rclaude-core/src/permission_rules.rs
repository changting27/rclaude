//! Permission rule parser and shell rule matching.

use std::path::Path;

use crate::permissions::{PermissionBehavior, PermissionRule, PermissionRuleSource};

/// Parsed permission rule value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRuleValue {
    pub tool_name: String,
    pub rule_content: Option<String>,
}

/// Parsed shell permission rule (discriminated union).
#[derive(Debug, Clone)]
pub enum ShellPermissionRule {
    Exact { command: String },
    Prefix { prefix: String },
    Wildcard { pattern: String },
}

// ── Rule parsing ──

/// Parse a permission rule string like "Bash(npm install)" into components.
pub fn parse_rule_value(rule_string: &str) -> PermissionRuleValue {
    let open = find_first_unescaped(rule_string, '(');
    if open.is_none() {
        return PermissionRuleValue {
            tool_name: rule_string.to_string(),
            rule_content: None,
        };
    }
    let open = open.unwrap();

    let close = find_last_unescaped(rule_string, ')');
    if close.is_none() || close.unwrap() <= open || close.unwrap() != rule_string.len() - 1 {
        return PermissionRuleValue {
            tool_name: rule_string.to_string(),
            rule_content: None,
        };
    }
    let close = close.unwrap();

    let tool_name = &rule_string[..open];
    if tool_name.is_empty() {
        return PermissionRuleValue {
            tool_name: rule_string.to_string(),
            rule_content: None,
        };
    }

    let raw_content = &rule_string[open + 1..close];

    // Empty or wildcard → tool-wide rule
    if raw_content.is_empty() || raw_content == "*" {
        return PermissionRuleValue {
            tool_name: tool_name.to_string(),
            rule_content: None,
        };
    }

    let content = unescape_rule_content(raw_content);
    PermissionRuleValue {
        tool_name: tool_name.to_string(),
        rule_content: Some(content),
    }
}

/// Convert a rule value back to string format.
pub fn rule_value_to_string(value: &PermissionRuleValue) -> String {
    match &value.rule_content {
        None => value.tool_name.clone(),
        Some(content) => format!("{}({})", value.tool_name, escape_rule_content(content)),
    }
}

/// Escape parentheses and backslashes in rule content.
pub fn escape_rule_content(content: &str) -> String {
    content
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

/// Unescape parentheses and backslashes in rule content.
pub fn unescape_rule_content(content: &str) -> String {
    content
        .replace("\\(", "(")
        .replace("\\)", ")")
        .replace("\\\\", "\\")
}

fn find_first_unescaped(s: &str, ch: char) -> Option<usize> {
    let bytes = s.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == ch as u8 {
            let mut backslashes = 0;
            let mut j = i as isize - 1;
            while j >= 0 && bytes[j as usize] == b'\\' {
                backslashes += 1;
                j -= 1;
            }
            if backslashes % 2 == 0 {
                return Some(i);
            }
        }
    }
    None
}

fn find_last_unescaped(s: &str, ch: char) -> Option<usize> {
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).rev() {
        if bytes[i] == ch as u8 {
            let mut backslashes = 0;
            let mut j = i as isize - 1;
            while j >= 0 && bytes[j as usize] == b'\\' {
                backslashes += 1;
                j -= 1;
            }
            if backslashes % 2 == 0 {
                return Some(i);
            }
        }
    }
    None
}

// ── Shell rule matching ──

/// Parse a shell permission rule string into a structured rule.
pub fn parse_shell_rule(rule: &str) -> ShellPermissionRule {
    // Legacy :* prefix syntax
    if let Some(prefix) = rule.strip_suffix(":*") {
        return ShellPermissionRule::Prefix {
            prefix: prefix.to_string(),
        };
    }
    // Wildcard syntax (unescaped *)
    if has_wildcards(rule) {
        return ShellPermissionRule::Wildcard {
            pattern: rule.to_string(),
        };
    }
    ShellPermissionRule::Exact {
        command: rule.to_string(),
    }
}

/// Check if a pattern contains unescaped wildcards.
pub fn has_wildcards(pattern: &str) -> bool {
    if pattern.ends_with(":*") {
        return false;
    }
    let bytes = pattern.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i] == b'*' {
            let mut backslashes = 0;
            let mut j = i as isize - 1;
            while j >= 0 && bytes[j as usize] == b'\\' {
                backslashes += 1;
                j -= 1;
            }
            if backslashes % 2 == 0 {
                return true;
            }
        }
    }
    false
}

/// Match a command against a wildcard pattern.
pub fn match_wildcard_pattern(pattern: &str, command: &str) -> bool {
    let trimmed = pattern.trim();

    // Process escape sequences
    let mut processed = String::new();
    let mut i = 0;
    let bytes = trimmed.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'*' => {
                    processed.push_str("\x00STAR\x00");
                    i += 2;
                    continue;
                }
                b'\\' => {
                    processed.push_str("\x00BSLASH\x00");
                    i += 2;
                    continue;
                }
                _ => {}
            }
        }
        processed.push(bytes[i] as char);
        i += 1;
    }

    // Escape regex special chars except *
    let escaped = regex::escape(&processed).replace(r"\*", ".*");

    // Restore placeholders
    let regex_pattern = escaped
        .replace("\x00STAR\x00", r"\*")
        .replace("\x00BSLASH\x00", r"\\");

    // Trailing ' *' with single wildcard → optional args (match bare command too)
    let unescaped_stars = processed.matches('*').count();
    let final_pattern = if regex_pattern.ends_with(" .*") && unescaped_stars == 1 {
        format!("{}( .*)?", &regex_pattern[..regex_pattern.len() - 3])
    } else {
        regex_pattern
    };

    regex::Regex::new(&format!("^{final_pattern}$")).is_ok_and(|re| re.is_match(command))
}

/// Match a command against a parsed shell permission rule.
pub fn match_shell_rule(rule: &ShellPermissionRule, command: &str) -> bool {
    match rule {
        ShellPermissionRule::Exact { command: rule_cmd } => command == rule_cmd,
        ShellPermissionRule::Prefix { prefix } => {
            command == prefix || command.starts_with(&format!("{prefix} "))
        }
        ShellPermissionRule::Wildcard { pattern } => match_wildcard_pattern(pattern, command),
    }
}

// ── Load rules from settings files ──

/// Load permission rules from a settings.json file.
pub fn load_rules_from_settings(path: &Path, source: PermissionRuleSource) -> Vec<PermissionRule> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let settings: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return vec![],
    };

    let mut rules = Vec::new();

    // Parse allowedTools
    if let Some(arr) = settings.get("allowedTools").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(s) = item.as_str() {
                let parsed = parse_rule_value(s);
                rules.push(PermissionRule {
                    source,
                    behavior: PermissionBehavior::Allow,
                    tool_name: parsed.tool_name,
                    rule_content: parsed.rule_content,
                });
            }
        }
    }

    // Parse deniedTools
    if let Some(arr) = settings.get("deniedTools").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(s) = item.as_str() {
                let parsed = parse_rule_value(s);
                rules.push(PermissionRule {
                    source,
                    behavior: PermissionBehavior::Deny,
                    tool_name: parsed.tool_name,
                    rule_content: parsed.rule_content,
                });
            }
        }
    }

    rules
}

/// Load all permission rules from all settings sources.
pub fn load_all_rules(cwd: &Path) -> Vec<PermissionRule> {
    let mut rules = Vec::new();

    // Managed (/etc/claude-code/.claude/settings.json)
    rules.extend(load_rules_from_settings(
        Path::new("/etc/claude-code/.claude/settings.json"),
        PermissionRuleSource::Managed,
    ));

    // User (~/.claude/settings.json)
    if let Some(home) = dirs::home_dir() {
        rules.extend(load_rules_from_settings(
            &home.join(".claude/settings.json"),
            PermissionRuleSource::UserSettings,
        ));
    }

    // Project (.claude/settings.json)
    rules.extend(load_rules_from_settings(
        &cwd.join(".claude/settings.json"),
        PermissionRuleSource::ProjectSettings,
    ));

    // Local (.claude/settings.local.json)
    rules.extend(load_rules_from_settings(
        &cwd.join(".claude/settings.local.json"),
        PermissionRuleSource::LocalSettings,
    ));

    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_tool() {
        let v = parse_rule_value("Bash");
        assert_eq!(v.tool_name, "Bash");
        assert_eq!(v.rule_content, None);
    }

    #[test]
    fn test_parse_tool_with_content() {
        let v = parse_rule_value("Bash(npm install)");
        assert_eq!(v.tool_name, "Bash");
        assert_eq!(v.rule_content, Some("npm install".into()));
    }

    #[test]
    fn test_parse_tool_with_escaped_parens() {
        let v = parse_rule_value(r"Bash(python -c print\(1\))");
        assert_eq!(v.tool_name, "Bash");
        assert_eq!(v.rule_content, Some("python -c print(1)".into()));
    }

    #[test]
    fn test_parse_wildcard_content() {
        let v = parse_rule_value("Bash(*)");
        assert_eq!(v.tool_name, "Bash");
        assert_eq!(v.rule_content, None); // * = tool-wide
    }

    #[test]
    fn test_parse_empty_parens() {
        let v = parse_rule_value("Bash()");
        assert_eq!(v.tool_name, "Bash");
        assert_eq!(v.rule_content, None);
    }

    #[test]
    fn test_roundtrip() {
        let v = PermissionRuleValue {
            tool_name: "Bash".into(),
            rule_content: Some("npm install".into()),
        };
        let s = rule_value_to_string(&v);
        assert_eq!(s, "Bash(npm install)");
        let v2 = parse_rule_value(&s);
        assert_eq!(v, v2);
    }

    #[test]
    fn test_roundtrip_with_parens() {
        let v = PermissionRuleValue {
            tool_name: "Bash".into(),
            rule_content: Some("print(1)".into()),
        };
        let s = rule_value_to_string(&v);
        let v2 = parse_rule_value(&s);
        assert_eq!(v, v2);
    }

    #[test]
    fn test_shell_rule_exact() {
        let rule = parse_shell_rule("npm install");
        assert!(match_shell_rule(&rule, "npm install"));
        assert!(!match_shell_rule(&rule, "npm test"));
    }

    #[test]
    fn test_shell_rule_prefix() {
        let rule = parse_shell_rule("npm:*");
        assert!(match_shell_rule(&rule, "npm install"));
        assert!(match_shell_rule(&rule, "npm test"));
        assert!(match_shell_rule(&rule, "npm"));
        assert!(!match_shell_rule(&rule, "npx test"));
    }

    #[test]
    fn test_shell_rule_wildcard() {
        let rule = parse_shell_rule("git *");
        assert!(match_shell_rule(&rule, "git status"));
        assert!(match_shell_rule(&rule, "git push --force"));
        assert!(match_shell_rule(&rule, "git")); // trailing ' *' is optional
        assert!(!match_shell_rule(&rule, "npm install"));
    }

    #[test]
    fn test_wildcard_middle() {
        let rule = parse_shell_rule("npm * --save");
        assert!(match_shell_rule(&rule, "npm install --save"));
        assert!(match_shell_rule(&rule, "npm uninstall lodash --save"));
        assert!(!match_shell_rule(&rule, "npm install"));
    }

    #[test]
    fn test_escaped_star() {
        assert!(!has_wildcards(r"echo \*"));
        assert!(has_wildcards("echo *"));
    }
}
