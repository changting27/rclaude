use serde::{Deserialize, Serialize};

/// Permission mode for tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    Default,
    Auto,
    BypassPermissions,
    Plan,
}

/// Result of a permission check.
#[derive(Debug, Clone)]
pub enum PermissionResult {
    Allowed,
    Denied(String),
    NeedApproval {
        description: String,
        risk: RiskLevel,
    },
}

/// Risk level for permission decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    None,
    Low,
    Medium,
    High,
}

/// Permission rule behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionBehavior {
    Allow,
    Deny,
    Ask,
}

/// A single permission rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Where this rule came from.
    pub source: PermissionRuleSource,
    /// What the rule does.
    pub behavior: PermissionBehavior,
    /// Tool name this rule applies to.
    pub tool_name: String,
    /// Optional content pattern (e.g., path glob, command pattern).
    #[serde(default)]
    pub rule_content: Option<String>,
}

/// Where a permission rule originates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionRuleSource {
    /// From ~/.claude/settings.json
    UserSettings,
    /// From .claude/settings.json in project
    ProjectSettings,
    /// From .claude/settings.local.json
    LocalSettings,
    /// From current session only
    Session,
    /// From CLI argument
    CliArg,
    /// From managed config (/etc/claude-code/)
    Managed,
}

/// Read-only tools that never need permission.
const READ_ONLY_TOOLS: &[&str] = &[
    "Read",
    "Glob",
    "Grep",
    "TaskList",
    "TaskGet",
    "TaskOutput",
    "CronList",
    "WebSearch",
    "WebFetch",
    "ListMcpResources",
    "ReadMcpResource",
    "LSP",
    "ToolSearch",
    "Brief",
    "Config",
];

/// Tools that modify the filesystem or run commands.
const WRITE_TOOLS: &[&str] = &["Write", "Edit", "NotebookEdit", "Bash", "PowerShell"];

/// High-risk tools.
const HIGH_RISK_TOOLS: &[&str] = &["Agent"];

/// Classify the risk level of a tool.
pub fn classify_tool_risk(tool_name: &str) -> RiskLevel {
    if READ_ONLY_TOOLS.contains(&tool_name) {
        RiskLevel::None
    } else if HIGH_RISK_TOOLS.contains(&tool_name) {
        RiskLevel::High
    } else if WRITE_TOOLS.contains(&tool_name) {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    }
}

/// Tools that are safe in auto mode and don't need classifier checking.
const AUTO_MODE_SAFE_TOOLS: &[&str] = &[
    // Read-only file operations
    "Read",
    // Search / read-only
    "Grep",
    "Glob",
    "LSP",
    "ToolSearch",
    "ListMcpResources",
    "ReadMcpResource",
    // Task management (metadata only)
    "TodoWrite",
    "TaskCreate",
    "TaskGet",
    "TaskUpdate",
    "TaskList",
    "TaskStop",
    "TaskOutput",
    // Plan mode / UI
    "AskUser",
    "EnterPlanMode",
    "ExitPlanMode",
    // Swarm coordination
    "TeamCreate",
    "TeamDelete",
    "SendMessage",
    // Sleep (no side effects)
    "Sleep",
    // Agent (has own permission checks)
    "Agent",
    // Skill (read-only discovery)
    "Skill",
];

/// Check if a tool is in the auto-mode safe allowlist.
pub fn is_auto_mode_safe_tool(tool_name: &str) -> bool {
    AUTO_MODE_SAFE_TOOLS.contains(&tool_name)
}

/// Check if a tool is allowed, considering permission mode and rules.
pub fn check_permission(tool_name: &str, mode: PermissionMode) -> PermissionResult {
    check_permission_with_rules(tool_name, mode, &[], None)
}

/// Check permission with explicit rules and optional command content.
pub fn check_permission_with_rules(
    tool_name: &str,
    mode: PermissionMode,
    rules: &[PermissionRule],
    content: Option<&str>,
) -> PermissionResult {
    // 1. Check explicit rules first (highest priority)
    if let Some(result) = evaluate_rules(tool_name, rules, content) {
        return result;
    }

    // 2. Fall back to mode-based check
    let risk = classify_tool_risk(tool_name);

    match mode {
        PermissionMode::BypassPermissions => PermissionResult::Allowed,
        PermissionMode::Auto => {
            // Auto mode: safe tools are auto-allowed, others need classification.
            if is_auto_mode_safe_tool(tool_name) || risk == RiskLevel::None {
                PermissionResult::Allowed
            } else {
                // For write tools in CWD, auto-allow (acceptEdits fast path).
                // For others, fall through to ask (LLM classifier not yet implemented).
                PermissionResult::NeedApproval {
                    description: format!(
                        "Auto mode: '{tool_name}' needs approval (not in safe list)"
                    ),
                    risk,
                }
            }
        }
        PermissionMode::Plan => {
            if risk == RiskLevel::None {
                PermissionResult::Allowed
            } else {
                PermissionResult::Denied(format!(
                    "Tool '{tool_name}' is not allowed in plan mode (read-only)"
                ))
            }
        }
        PermissionMode::Default => match risk {
            RiskLevel::None => PermissionResult::Allowed,
            _ => PermissionResult::NeedApproval {
                description: format!("Tool '{tool_name}' wants to modify your system"),
                risk,
            },
        },
    }
}

/// Evaluate explicit permission rules against a tool invocation.
/// Returns None if no rule matches (fall through to mode-based check).
fn evaluate_rules(
    tool_name: &str,
    rules: &[PermissionRule],
    content: Option<&str>,
) -> Option<PermissionResult> {
    // Rules are evaluated in order; first match wins.
    // More specific rules (with content) are checked before general ones.
    for rule in rules {
        if !rule_matches_tool(&rule.tool_name, tool_name) {
            continue;
        }

        // If rule has content pattern, check it
        if let Some(ref pattern) = rule.rule_content {
            if let Some(cmd_content) = content {
                if !content_matches_pattern(cmd_content, pattern) {
                    continue;
                }
            } else {
                continue; // Rule requires content but none provided
            }
        }

        // Rule matches
        return Some(match rule.behavior {
            PermissionBehavior::Allow => PermissionResult::Allowed,
            PermissionBehavior::Deny => PermissionResult::Denied(format!(
                "Denied by {} rule for '{}'",
                source_name(rule.source),
                tool_name
            )),
            PermissionBehavior::Ask => PermissionResult::NeedApproval {
                description: format!(
                    "Rule from {} requires approval for '{}'",
                    source_name(rule.source),
                    tool_name
                ),
                risk: classify_tool_risk(tool_name),
            },
        });
    }
    None
}

/// Check if a rule's tool pattern matches a tool name.
/// Supports exact match and glob patterns (e.g., "Bash(*)" matches "Bash").
fn rule_matches_tool(pattern: &str, tool_name: &str) -> bool {
    if pattern == tool_name {
        return true;
    }
    // "Bash(*)" style — match tool name prefix
    if let Some(prefix) = pattern.strip_suffix("(*)") {
        return tool_name == prefix;
    }
    // "Bash(command:*)" style — match with content qualifier
    if pattern.contains('(') && pattern.ends_with(')') {
        let base = pattern.split('(').next().unwrap_or("");
        return tool_name == base;
    }
    false
}

/// Check if command content matches a rule's content pattern.
/// Supports glob-like patterns: `*` matches anything, prefix/suffix matching.
fn content_matches_pattern(content: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return content.ends_with(suffix);
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return content.starts_with(prefix);
    }
    content == pattern
}

fn source_name(source: PermissionRuleSource) -> &'static str {
    match source {
        PermissionRuleSource::UserSettings => "user settings",
        PermissionRuleSource::ProjectSettings => "project settings",
        PermissionRuleSource::LocalSettings => "local settings",
        PermissionRuleSource::Session => "session",
        PermissionRuleSource::CliArg => "CLI argument",
        PermissionRuleSource::Managed => "managed config",
    }
}

/// Prompt the user for permission (blocking stdin read).
/// If `persist` is true, saves the decision to project settings.
/// Q06+U08: Interactive permission prompt with formatting and persistence.
pub fn prompt_user_permission(description: &str) -> bool {
    prompt_user_permission_with_persist(description, None).0
}

pub fn prompt_user_permission_with_persist(
    description: &str,
    tool_name: Option<&str>,
) -> (bool, Option<PermissionRule>) {
    use crate::permission_prompt::{show_permission_select, PermissionChoice};

    let display_name = tool_name.unwrap_or("Tool");
    let choice = show_permission_select(display_name, description);

    match choice {
        PermissionChoice::AllowOnce => (true, None),
        PermissionChoice::AllowAlways => {
            let rule = tool_name.map(|name| PermissionRule {
                source: PermissionRuleSource::Session,
                behavior: PermissionBehavior::Allow,
                tool_name: name.to_string(),
                rule_content: None,
            });
            (true, rule)
        }
        PermissionChoice::Deny => (false, None),
        PermissionChoice::DenyAlways => {
            let rule = tool_name.map(|name| PermissionRule {
                source: PermissionRuleSource::Session,
                behavior: PermissionBehavior::Deny,
                tool_name: name.to_string(),
                rule_content: None,
            });
            (false, rule)
        }
    }
}

/// Persist a permission rule to the project settings file.
pub fn persist_permission_rule(cwd: &std::path::Path, rule: &PermissionRule) {
    let settings_path = cwd.join(".claude/settings.json");
    let mut settings: serde_json::Value = std::fs::read_to_string(&settings_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    let key = match rule.behavior {
        PermissionBehavior::Allow => "allowedTools",
        PermissionBehavior::Deny => "deniedTools",
        PermissionBehavior::Ask => return, // Don't persist "ask"
    };

    let entry = if let Some(ref content) = rule.rule_content {
        format!("{}:{}", rule.tool_name, content)
    } else {
        rule.tool_name.clone()
    };

    let arr = settings
        .as_object_mut()
        .unwrap()
        .entry(key)
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut();

    if let Some(arr) = arr {
        let val = serde_json::Value::String(entry);
        if !arr.contains(&val) {
            arr.push(val);
        }
    }

    if let Some(parent) = settings_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        &settings_path,
        serde_json::to_string_pretty(&settings).unwrap_or_default(),
    );
}

/// Load permission rules from config's allowed_tools/denied_tools.
pub fn rules_from_config(config: &crate::config::Config) -> Vec<PermissionRule> {
    let mut rules = Vec::new();
    for tool in &config.allowed_tools {
        rules.push(PermissionRule {
            source: PermissionRuleSource::ProjectSettings,
            behavior: PermissionBehavior::Allow,
            tool_name: tool.clone(),
            rule_content: None,
        });
    }
    for tool in &config.denied_tools {
        rules.push(PermissionRule {
            source: PermissionRuleSource::ProjectSettings,
            behavior: PermissionBehavior::Deny,
            tool_name: tool.clone(),
            rule_content: None,
        });
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_read_tools() {
        assert_eq!(classify_tool_risk("Read"), RiskLevel::None);
        assert_eq!(classify_tool_risk("Glob"), RiskLevel::None);
        assert_eq!(classify_tool_risk("Grep"), RiskLevel::None);
    }

    #[test]
    fn test_classify_write_tools() {
        assert_eq!(classify_tool_risk("Write"), RiskLevel::Medium);
        assert_eq!(classify_tool_risk("Edit"), RiskLevel::Medium);
        assert_eq!(classify_tool_risk("Bash"), RiskLevel::Medium);
    }

    #[test]
    fn test_classify_high_risk() {
        assert_eq!(classify_tool_risk("Agent"), RiskLevel::High);
    }

    #[test]
    fn test_bypass_allows_all() {
        assert!(matches!(
            check_permission("Bash", PermissionMode::BypassPermissions),
            PermissionResult::Allowed
        ));
    }

    #[test]
    fn test_plan_blocks_writes() {
        assert!(matches!(
            check_permission("Read", PermissionMode::Plan),
            PermissionResult::Allowed
        ));
        assert!(matches!(
            check_permission("Bash", PermissionMode::Plan),
            PermissionResult::Denied(_)
        ));
    }

    #[test]
    fn test_auto_mode_safe_tools() {
        // Safe tools are auto-allowed
        assert!(matches!(
            check_permission("Agent", PermissionMode::Auto),
            PermissionResult::Allowed
        ));
        assert!(matches!(
            check_permission("Read", PermissionMode::Auto),
            PermissionResult::Allowed
        ));
        assert!(matches!(
            check_permission("Grep", PermissionMode::Auto),
            PermissionResult::Allowed
        ));
        // Risky tools need explicit approval
        assert!(matches!(
            check_permission("Bash", PermissionMode::Auto),
            PermissionResult::NeedApproval { .. }
        ));
    }

    #[test]
    fn test_default_asks_for_writes() {
        assert!(matches!(
            check_permission("Read", PermissionMode::Default),
            PermissionResult::Allowed
        ));
        assert!(matches!(
            check_permission("Bash", PermissionMode::Default),
            PermissionResult::NeedApproval { .. }
        ));
    }

    #[test]
    fn test_explicit_allow_rule_overrides_mode() {
        let rules = vec![PermissionRule {
            source: PermissionRuleSource::ProjectSettings,
            behavior: PermissionBehavior::Allow,
            tool_name: "Bash".into(),
            rule_content: None,
        }];
        let result = check_permission_with_rules("Bash", PermissionMode::Default, &rules, None);
        assert!(matches!(result, PermissionResult::Allowed));
    }

    #[test]
    fn test_explicit_deny_rule() {
        let rules = vec![PermissionRule {
            source: PermissionRuleSource::UserSettings,
            behavior: PermissionBehavior::Deny,
            tool_name: "Bash".into(),
            rule_content: None,
        }];
        let result =
            check_permission_with_rules("Bash", PermissionMode::BypassPermissions, &rules, None);
        assert!(matches!(result, PermissionResult::Denied(_)));
    }

    #[test]
    fn test_content_pattern_matching() {
        let rules = vec![PermissionRule {
            source: PermissionRuleSource::ProjectSettings,
            behavior: PermissionBehavior::Allow,
            tool_name: "Bash".into(),
            rule_content: Some("npm *".into()),
        }];
        // Matches
        let r = check_permission_with_rules(
            "Bash",
            PermissionMode::Default,
            &rules,
            Some("npm install"),
        );
        assert!(matches!(r, PermissionResult::Allowed));
        // Doesn't match — falls through to mode
        let r =
            check_permission_with_rules("Bash", PermissionMode::Default, &rules, Some("rm -rf /"));
        assert!(matches!(r, PermissionResult::NeedApproval { .. }));
    }

    #[test]
    fn test_rule_matches_tool_glob() {
        assert!(rule_matches_tool("Bash(*)", "Bash"));
        assert!(rule_matches_tool("Bash(command:npm*)", "Bash"));
        assert!(!rule_matches_tool("Write", "Bash"));
    }

    #[test]
    fn test_rules_from_config() {
        let config = crate::config::Config {
            allowed_tools: vec!["Bash".into(), "Write".into()],
            denied_tools: vec!["Agent".into()],
            ..Default::default()
        };
        let rules = rules_from_config(&config);
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].behavior, PermissionBehavior::Allow);
        assert_eq!(rules[2].behavior, PermissionBehavior::Deny);
    }
}
