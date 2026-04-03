//! Auto-mode classifier for permission decisions.
//!
//! In auto mode, the classifier decides whether to allow/deny tool actions
//! without user prompting, based on rules and command analysis.

use crate::denial_tracking::DenialTrackingState;

/// Auto-mode rules configuration.
#[derive(Debug, Clone, Default)]
pub struct AutoModeRules {
    /// Commands/patterns that are always allowed.
    pub allow: Vec<String>,
    /// Commands/patterns that should be denied (soft — falls back to ask).
    pub soft_deny: Vec<String>,
}

/// Auto-mode classifier state.
pub struct AutoModeClassifier {
    rules: AutoModeRules,
    denial_state: DenialTrackingState,
}

impl AutoModeClassifier {
    pub fn new(rules: AutoModeRules) -> Self {
        Self {
            rules,
            denial_state: DenialTrackingState::new(),
        }
    }

    /// Classify a tool action in auto mode.
    /// Returns Allow if the action matches allow rules or is safe,
    /// Deny if it matches deny rules, or Ask if uncertain.
    pub fn classify(
        &mut self,
        tool_name: &str,
        command: Option<&str>,
        is_read_only: bool,
    ) -> AutoModeDecision {
        // If too many denials, fall back to prompting
        if self.denial_state.should_fallback_to_prompting() {
            return AutoModeDecision::Ask("Too many denials, falling back to prompting".into());
        }

        // Read-only tools are always allowed
        if is_read_only {
            self.denial_state.record_success();
            return AutoModeDecision::Allow;
        }

        // Check allow rules
        if let Some(cmd) = command {
            for pattern in &self.rules.allow {
                if matches_rule(cmd, pattern) {
                    self.denial_state.record_success();
                    return AutoModeDecision::Allow;
                }
            }
        }

        // Check deny rules
        if let Some(cmd) = command {
            for pattern in &self.rules.soft_deny {
                if matches_rule(cmd, pattern) {
                    self.denial_state.record_denial();
                    return AutoModeDecision::Deny(format!("Matches deny rule: {pattern}"));
                }
            }
        }

        // For Bash commands, check against allow rules (already handled above)
        // Additional heuristic: simple commands without pipes/redirects are likely safe
        if tool_name == "Bash" {
            if let Some(cmd) = command {
                let simple = !cmd.contains('|')
                    && !cmd.contains('>')
                    && !cmd.contains('<')
                    && !cmd.contains('$')
                    && !cmd.contains('`');
                if simple {
                    self.denial_state.record_success();
                    return AutoModeDecision::Allow;
                }
            }
        }

        // Default: allow in auto mode (matching original's permissive default)
        self.denial_state.record_success();
        AutoModeDecision::Allow
    }

    /// Record a user denial (when the user rejects an auto-approved action).
    pub fn record_user_denial(&mut self) {
        self.denial_state.record_denial();
    }
}

/// Decision from the auto-mode classifier.
#[derive(Debug, Clone)]
pub enum AutoModeDecision {
    Allow,
    Deny(String),
    Ask(String),
}

/// Check if a command matches a rule pattern.
fn matches_rule(command: &str, pattern: &str) -> bool {
    // Exact match
    if command == pattern {
        return true;
    }
    // Prefix match (pattern ends with *)
    if let Some(prefix) = pattern.strip_suffix('*') {
        if command.starts_with(prefix) {
            return true;
        }
    }
    // Contains match (pattern starts and ends with *)
    if pattern.starts_with('*') && pattern.ends_with('*') && pattern.len() > 2 {
        let inner = &pattern[1..pattern.len() - 1];
        if command.contains(inner) {
            return true;
        }
    }
    false
}

/// Default allow rules for auto mode (matching getDefaultExternalAutoModeRules).
pub fn default_allow_rules() -> Vec<String> {
    vec![
        "git status".into(),
        "git diff*".into(),
        "git log*".into(),
        "git branch*".into(),
        "git show*".into(),
        "ls*".into(),
        "cat*".into(),
        "head*".into(),
        "tail*".into(),
        "find*".into(),
        "grep*".into(),
        "rg*".into(),
        "pwd".into(),
        "echo*".into(),
        "wc*".into(),
        "npm test*".into(),
        "npm run test*".into(),
        "cargo test*".into(),
        "cargo check*".into(),
        "cargo clippy*".into(),
        "python -m pytest*".into(),
        "go test*".into(),
        "make test*".into(),
        "make check*".into(),
    ]
}

/// Default deny rules for auto mode.
pub fn default_deny_rules() -> Vec<String> {
    vec![
        "rm -rf /*".into(),
        "rm -rf /".into(),
        "git push --force*".into(),
        "git reset --hard*".into(),
        "chmod -R 777*".into(),
        "curl*|*sh".into(),
        "wget*|*sh".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_rule_exact() {
        assert!(matches_rule("git status", "git status"));
        assert!(!matches_rule("git push", "git status"));
    }

    #[test]
    fn test_matches_rule_prefix() {
        assert!(matches_rule("git log --oneline", "git log*"));
        assert!(matches_rule("npm test", "npm test*"));
        assert!(!matches_rule("npm install", "npm test*"));
    }

    #[test]
    fn test_classifier_read_only() {
        let mut c = AutoModeClassifier::new(AutoModeRules::default());
        assert!(matches!(
            c.classify("Read", None, true),
            AutoModeDecision::Allow
        ));
    }

    #[test]
    fn test_classifier_allow_rule() {
        let rules = AutoModeRules {
            allow: vec!["cargo test*".into()],
            soft_deny: vec![],
        };
        let mut c = AutoModeClassifier::new(rules);
        assert!(matches!(
            c.classify("Bash", Some("cargo test --all"), false),
            AutoModeDecision::Allow
        ));
    }

    #[test]
    fn test_classifier_deny_rule() {
        let rules = AutoModeRules {
            allow: vec![],
            soft_deny: vec!["rm -rf*".into()],
        };
        let mut c = AutoModeClassifier::new(rules);
        assert!(matches!(
            c.classify("Bash", Some("rm -rf /tmp/test"), false),
            AutoModeDecision::Deny(_)
        ));
    }

    #[test]
    fn test_denial_fallback() {
        let mut c = AutoModeClassifier::new(AutoModeRules {
            allow: vec![],
            soft_deny: vec!["*".into()],
        });
        for _ in 0..3 {
            c.classify("Bash", Some("dangerous"), false);
        }
        assert!(matches!(
            c.classify("Bash", Some("anything"), false),
            AutoModeDecision::Ask(_)
        ));
    }
}
