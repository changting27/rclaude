//! Hook system for lifecycle events.
//! Q05: Full hook types (command/prompt/http/agent), conditions, once flag.

use std::collections::HashMap;

/// Hook event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    Notification,
    SessionStart,
    SessionEnd,
    Stop,
    SubagentStart,
    SubagentStop,
    PreCompact,
    PostCompact,
    PermissionRequest,
    PermissionDenied,
    InstructionsLoaded,
}

impl HookEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::Notification => "Notification",
            Self::SessionStart => "SessionStart",
            Self::SessionEnd => "SessionEnd",
            Self::Stop => "Stop",
            Self::SubagentStart => "SubagentStart",
            Self::SubagentStop => "SubagentStop",
            Self::PreCompact => "PreCompact",
            Self::PostCompact => "PostCompact",
            Self::PermissionRequest => "PermissionRequest",
            Self::PermissionDenied => "PermissionDenied",
            Self::InstructionsLoaded => "InstructionsLoaded",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "PreToolUse" => Some(Self::PreToolUse),
            "PostToolUse" => Some(Self::PostToolUse),
            "Notification" => Some(Self::Notification),
            "SessionStart" => Some(Self::SessionStart),
            "SessionEnd" => Some(Self::SessionEnd),
            "Stop" => Some(Self::Stop),
            "SubagentStart" => Some(Self::SubagentStart),
            "SubagentStop" => Some(Self::SubagentStop),
            "PreCompact" => Some(Self::PreCompact),
            "PostCompact" => Some(Self::PostCompact),
            "PermissionRequest" => Some(Self::PermissionRequest),
            "PermissionDenied" => Some(Self::PermissionDenied),
            "InstructionsLoaded" => Some(Self::InstructionsLoaded),
            _ => None,
        }
    }
}

/// Q05: Hook type — command, prompt, http, or agent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum HookAction {
    #[serde(rename = "command")]
    Command {
        command: String,
        #[serde(default)]
        shell: Option<String>,
    },
    #[serde(rename = "prompt")]
    Prompt {
        prompt: String,
        #[serde(default)]
        model: Option<String>,
    },
    #[serde(rename = "http")]
    Http {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
    #[serde(rename = "agent")]
    Agent {
        prompt: String,
        #[serde(default)]
        model: Option<String>,
    },
}

/// A hook matcher: event filter + action + conditions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HookMatcher {
    /// The action to execute.
    #[serde(flatten)]
    pub action: HookAction,
    /// Optional tool name filter (for PreToolUse/PostToolUse).
    #[serde(default, rename = "matcher")]
    pub tool_name: Option<String>,
    /// Timeout in milliseconds.
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    /// Q05: Condition filter (permission rule syntax, e.g. "Bash(npm *)").
    #[serde(default, rename = "if")]
    pub condition: Option<String>,
    /// Q05: Only execute once per session.
    #[serde(default)]
    pub once: bool,
    /// Status message to display while hook runs.
    #[serde(default, rename = "statusMessage")]
    pub status_message: Option<String>,
}

fn default_timeout() -> u64 {
    10_000
}

#[derive(Debug, Clone)]
pub struct HookResult {
    pub hook_event: HookEvent,
    pub hook_name: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub outcome: HookOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookOutcome {
    Success,
    Error,
    Cancelled,
    Skipped,
}

/// Hook registry with once-tracking.
#[derive(Debug, Default, Clone)]
pub struct HookRegistry {
    hooks: HashMap<HookEvent, Vec<HookMatcher>>,
    /// Q05: Track which hooks have been executed (for `once` flag).
    executed_once: std::collections::HashSet<String>,
}

impl HookRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, event: HookEvent, matcher: HookMatcher) {
        self.hooks.entry(event).or_default().push(matcher);
    }

    pub fn get(&self, event: HookEvent) -> &[HookMatcher] {
        self.hooks.get(&event).map_or(&[], |v| v.as_slice())
    }

    /// Execute all hooks for an event with condition checking.
    pub async fn run(
        &mut self,
        event: HookEvent,
        cwd: &std::path::Path,
        env: &HashMap<String, String>,
    ) -> Vec<HookResult> {
        let matchers = self.hooks.get(&event).cloned().unwrap_or_default();
        let mut results = Vec::new();

        for matcher in &matchers {
            // Q05: Check `if` condition
            if let Some(ref condition) = matcher.condition {
                let tool_name = env.get("TOOL_NAME").map(|s| s.as_str()).unwrap_or("");
                let tool_input = env.get("TOOL_INPUT").map(|s| s.as_str()).unwrap_or("");
                if !check_condition(condition, tool_name, tool_input) {
                    results.push(HookResult {
                        hook_event: event,
                        hook_name: format_hook_name(&matcher.action),
                        stdout: String::new(),
                        stderr: String::new(),
                        exit_code: None,
                        outcome: HookOutcome::Skipped,
                    });
                    continue;
                }
            }

            // Q05: Check `once` flag
            let hook_key = format!("{:?}:{}", event, format_hook_name(&matcher.action));
            if matcher.once && self.executed_once.contains(&hook_key) {
                results.push(HookResult {
                    hook_event: event,
                    hook_name: format_hook_name(&matcher.action),
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: None,
                    outcome: HookOutcome::Skipped,
                });
                continue;
            }

            // Q05: Check tool name filter
            if let Some(ref filter) = matcher.tool_name {
                let tool_name = env.get("TOOL_NAME").map(|s| s.as_str()).unwrap_or("");
                if !filter.eq_ignore_ascii_case(tool_name) {
                    continue;
                }
            }

            let result = run_hook_action(event, matcher, cwd, env).await;
            if matcher.once && result.outcome == HookOutcome::Success {
                self.executed_once.insert(hook_key);
            }
            results.push(result);
        }
        results
    }

    pub fn load_from_settings(&mut self, settings: &serde_json::Value) {
        let hooks_val = match settings.get("hooks") {
            Some(v) => v,
            None => return,
        };
        let hooks_obj = match hooks_val.as_object() {
            Some(o) => o,
            None => return,
        };
        for (event_name, matchers_val) in hooks_obj {
            let event = match HookEvent::parse(event_name) {
                Some(e) => e,
                None => continue,
            };
            let matchers_arr = match matchers_val.as_array() {
                Some(a) => a,
                None => continue,
            };
            for matcher_val in matchers_arr {
                // Each matcher has a "matcher" field and "hooks" array
                let hooks_arr = match matcher_val.get("hooks").and_then(|h| h.as_array()) {
                    Some(a) => a,
                    None => continue,
                };
                let tool_filter = matcher_val
                    .get("matcher")
                    .and_then(|m| m.as_str())
                    .map(|s| s.to_string());

                for hook_def in hooks_arr {
                    if let Ok(mut hook) = serde_json::from_value::<HookMatcher>(hook_def.clone()) {
                        if hook.tool_name.is_none() {
                            hook.tool_name = tool_filter.clone();
                        }
                        self.register(event, hook);
                    }
                }
            }
        }
    }
}

/// Q05: Check if a condition matches the current tool invocation.
fn check_condition(condition: &str, tool_name: &str, tool_input: &str) -> bool {
    if let Some(paren) = condition.find('(') {
        let cond_tool = &condition[..paren];
        let pattern = condition[paren + 1..].trim_end_matches(')');
        if !cond_tool.eq_ignore_ascii_case(tool_name) {
            return false;
        }
        if pattern == "*" {
            return true;
        }
        // Glob-like: "npm *" matches "npm install", "prefix*" matches "prefix..."
        if let Some(prefix) = pattern.strip_suffix('*') {
            return tool_input.contains(prefix.trim());
        }
        tool_input.contains(pattern)
    } else {
        condition.eq_ignore_ascii_case(tool_name)
    }
}

fn format_hook_name(action: &HookAction) -> String {
    match action {
        HookAction::Command { command, .. } => command.clone(),
        HookAction::Prompt { prompt, .. } => format!("prompt:{}", &prompt[..prompt.len().min(30)]),
        HookAction::Http { url, .. } => format!("http:{url}"),
        HookAction::Agent { prompt, .. } => format!("agent:{}", &prompt[..prompt.len().min(30)]),
    }
}

async fn run_hook_action(
    event: HookEvent,
    matcher: &HookMatcher,
    cwd: &std::path::Path,
    env: &HashMap<String, String>,
) -> HookResult {
    let timeout = std::time::Duration::from_millis(matcher.timeout);
    let name = format_hook_name(&matcher.action);

    match &matcher.action {
        HookAction::Command { command, .. } => {
            run_command_hook(event, &name, command, cwd, env, timeout).await
        }
        HookAction::Http { url, headers } => {
            run_http_hook(event, &name, url, headers, timeout).await
        }
        // Prompt and Agent hooks return the prompt text (caller handles execution)
        HookAction::Prompt { prompt, .. } | HookAction::Agent { prompt, .. } => HookResult {
            hook_event: event,
            hook_name: name,
            stdout: prompt.clone(),
            stderr: String::new(),
            exit_code: Some(0),
            outcome: HookOutcome::Success,
        },
    }
}

async fn run_command_hook(
    event: HookEvent,
    name: &str,
    command: &str,
    cwd: &std::path::Path,
    env: &HashMap<String, String>,
    timeout: std::time::Duration,
) -> HookResult {
    let spawn_result = tokio::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(cwd)
        .envs(env)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    let child = match spawn_result {
        Ok(c) => c,
        Err(e) => {
            return HookResult {
                hook_event: event,
                hook_name: name.to_string(),
                stdout: String::new(),
                stderr: e.to_string(),
                exit_code: None,
                outcome: HookOutcome::Error,
            };
        }
    };

    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(output)) => HookResult {
            hook_event: event,
            hook_name: name.to_string(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
            outcome: if output.status.success() {
                HookOutcome::Success
            } else {
                HookOutcome::Error
            },
        },
        Ok(Err(e)) => HookResult {
            hook_event: event,
            hook_name: name.to_string(),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: None,
            outcome: HookOutcome::Error,
        },
        Err(_) => HookResult {
            hook_event: event,
            hook_name: name.to_string(),
            stdout: String::new(),
            stderr: format!("Hook timed out after {}ms", timeout.as_millis()),
            exit_code: None,
            outcome: HookOutcome::Cancelled,
        },
    }
}

/// Q05: HTTP hook — POST to URL with env as JSON body.
async fn run_http_hook(
    event: HookEvent,
    name: &str,
    url: &str,
    headers: &HashMap<String, String>,
    timeout: std::time::Duration,
) -> HookResult {
    let client = reqwest::Client::new();
    let mut req = client.post(url).timeout(timeout);
    for (k, v) in headers {
        req = req.header(k, v);
    }

    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            HookResult {
                hook_event: event,
                hook_name: name.to_string(),
                stdout: body,
                stderr: String::new(),
                exit_code: Some(status.as_u16() as i32),
                outcome: if status.is_success() {
                    HookOutcome::Success
                } else {
                    HookOutcome::Error
                },
            }
        }
        Err(e) => HookResult {
            hook_event: event,
            hook_name: name.to_string(),
            stdout: String::new(),
            stderr: e.to_string(),
            exit_code: None,
            outcome: HookOutcome::Error,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_registry() {
        let mut reg = HookRegistry::new();
        reg.register(
            HookEvent::PreToolUse,
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
        assert_eq!(reg.get(HookEvent::PreToolUse).len(), 1);
        assert_eq!(reg.get(HookEvent::PostToolUse).len(), 0);
    }

    #[test]
    fn test_check_condition_simple() {
        assert!(check_condition("Bash", "Bash", ""));
        assert!(!check_condition("Bash", "Read", ""));
    }

    #[test]
    fn test_check_condition_with_pattern() {
        assert!(check_condition("Bash(npm *)", "Bash", "npm install"));
        assert!(!check_condition("Bash(npm *)", "Bash", "cargo build"));
        assert!(check_condition("Bash(*)", "Bash", "anything"));
    }

    #[tokio::test]
    async fn test_run_hook_success() {
        let mut reg = HookRegistry::new();
        reg.register(
            HookEvent::SessionStart,
            HookMatcher {
                action: HookAction::Command {
                    command: "echo hello".into(),
                    shell: None,
                },
                tool_name: None,
                timeout: 5000,
                condition: None,
                once: false,
                status_message: None,
            },
        );
        let results = reg
            .run(
                HookEvent::SessionStart,
                std::path::Path::new("/tmp"),
                &HashMap::new(),
            )
            .await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].outcome, HookOutcome::Success);
    }

    #[tokio::test]
    async fn test_once_flag() {
        let mut reg = HookRegistry::new();
        reg.register(
            HookEvent::Stop,
            HookMatcher {
                action: HookAction::Command {
                    command: "echo once".into(),
                    shell: None,
                },
                tool_name: None,
                timeout: 5000,
                condition: None,
                once: true,
                status_message: None,
            },
        );
        let r1 = reg
            .run(
                HookEvent::Stop,
                std::path::Path::new("/tmp"),
                &HashMap::new(),
            )
            .await;
        assert_eq!(r1[0].outcome, HookOutcome::Success);

        let r2 = reg
            .run(
                HookEvent::Stop,
                std::path::Path::new("/tmp"),
                &HashMap::new(),
            )
            .await;
        assert_eq!(r2[0].outcome, HookOutcome::Skipped);
    }
}
