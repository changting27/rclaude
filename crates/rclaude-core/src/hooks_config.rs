//! Hooks configuration manager.

use crate::hooks::{HookAction, HookEvent, HookMatcher};
use std::collections::HashMap;

/// Metadata about a hook event type.
#[derive(Debug, Clone)]
pub struct HookEventMetadata {
    pub summary: &'static str,
    pub description: &'static str,
}

pub fn get_hook_event_metadata() -> HashMap<HookEvent, HookEventMetadata> {
    let mut m = HashMap::new();
    m.insert(
        HookEvent::PreToolUse,
        HookEventMetadata {
            summary: "Before tool execution",
            description: "Exit 0: continue. Exit 2: block tool call.",
        },
    );
    m.insert(
        HookEvent::PostToolUse,
        HookEventMetadata {
            summary: "After tool execution",
            description: "Exit 0: continue. Exit 2: show to model.",
        },
    );
    m.insert(
        HookEvent::SessionStart,
        HookEventMetadata {
            summary: "When session starts",
            description: "Runs once at session initialization.",
        },
    );
    m.insert(
        HookEvent::SessionEnd,
        HookEventMetadata {
            summary: "When session ends",
            description: "Runs during graceful shutdown.",
        },
    );
    m.insert(
        HookEvent::Stop,
        HookEventMetadata {
            summary: "When model stops",
            description: "Runs when model produces end_turn.",
        },
    );
    m
}

/// Session-scoped hooks registered at runtime.
#[derive(Debug, Default)]
pub struct SessionHooks {
    hooks: HashMap<HookEvent, Vec<SessionHookEntry>>,
}

#[derive(Debug, Clone)]
pub struct SessionHookEntry {
    pub matcher: String,
    pub hook: HookMatcher,
}

impl SessionHooks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, event: HookEvent, matcher: &str, hook: HookMatcher) {
        self.hooks.entry(event).or_default().push(SessionHookEntry {
            matcher: matcher.to_string(),
            hook,
        });
    }

    pub fn remove(&mut self, event: HookEvent, matcher: &str) {
        if let Some(entries) = self.hooks.get_mut(&event) {
            entries.retain(|e| e.matcher != matcher);
        }
    }

    pub fn get(&self, event: HookEvent) -> Vec<&HookMatcher> {
        self.hooks
            .get(&event)
            .map(|entries| entries.iter().map(|e| &e.hook).collect())
            .unwrap_or_default()
    }

    pub fn clear(&mut self) {
        self.hooks.clear();
    }

    pub fn count(&self) -> usize {
        self.hooks.values().map(|v| v.len()).sum()
    }
}

/// Load hooks from settings JSON (legacy format compatibility).
pub fn load_hooks_from_settings(
    settings: &serde_json::Value,
) -> HashMap<HookEvent, Vec<HookMatcher>> {
    let mut result = HashMap::new();
    let hooks = match settings.get("hooks").and_then(|v| v.as_object()) {
        Some(h) => h,
        None => return result,
    };

    for (event_name, matchers) in hooks {
        let event = match HookEvent::parse(event_name) {
            Some(e) => e,
            None => continue,
        };

        if let Some(arr) = matchers.as_array() {
            let mut hooks_for_event = Vec::new();
            for matcher in arr {
                // Try new format first (direct HookMatcher)
                if let Ok(hook) = serde_json::from_value::<HookMatcher>(matcher.clone()) {
                    hooks_for_event.push(hook);
                    continue;
                }
                // Legacy format: { "matcher": "Bash", "hooks": [{"type":"command","command":"echo"}] }
                if let Some(obj) = matcher.as_object() {
                    let tool_filter = obj
                        .get("matcher")
                        .and_then(|m| m.as_str())
                        .map(|s| s.to_string());
                    if let Some(hook_arr) = obj.get("hooks").and_then(|h| h.as_array()) {
                        for hook_def in hook_arr {
                            if let Ok(mut hook) =
                                serde_json::from_value::<HookMatcher>(hook_def.clone())
                            {
                                if hook.tool_name.is_none() {
                                    hook.tool_name = tool_filter.clone();
                                }
                                hooks_for_event.push(hook);
                            } else if let Some(cmd) =
                                hook_def.get("command").and_then(|v| v.as_str())
                            {
                                // Bare command string
                                hooks_for_event.push(HookMatcher {
                                    action: HookAction::Command {
                                        command: cmd.to_string(),
                                        shell: None,
                                    },
                                    tool_name: tool_filter.clone(),
                                    timeout: hook_def
                                        .get("timeout")
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(10_000),
                                    condition: None,
                                    once: false,
                                    status_message: None,
                                });
                            }
                        }
                    }
                }
            }
            if !hooks_for_event.is_empty() {
                result.insert(event, hooks_for_event);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_hooks() {
        let mut sh = SessionHooks::new();
        sh.add(
            HookEvent::PreToolUse,
            "Bash",
            HookMatcher {
                action: HookAction::Command {
                    command: "echo pre".into(),
                    shell: None,
                },
                tool_name: Some("Bash".into()),
                timeout: 5000,
                condition: None,
                once: false,
                status_message: None,
            },
        );
        assert_eq!(sh.count(), 1);
        assert_eq!(sh.get(HookEvent::PreToolUse).len(), 1);
        sh.remove(HookEvent::PreToolUse, "Bash");
        assert_eq!(sh.count(), 0);
    }

    #[test]
    fn test_hook_event_metadata() {
        let meta = get_hook_event_metadata();
        assert!(meta.contains_key(&HookEvent::PreToolUse));
    }

    #[test]
    fn test_load_hooks_legacy_format() {
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [{"type": "command", "command": "echo pre"}]
                }]
            }
        });
        let hooks = load_hooks_from_settings(&settings);
        assert!(hooks.contains_key(&HookEvent::PreToolUse));
    }
}
