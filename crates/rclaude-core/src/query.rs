//! Query engine: the core agentic loop with error recovery.
//! Behaviors:
//! - prompt-too-long → reactive compact → retry
//! - max_output_tokens → auto-continue (up to 3x)
//! - 529 overload → fallback model
//! - tool result budget enforcement
//! - streaming with retry

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::message::{ContentBlock, Message, Role};
use crate::state::AppState;
use crate::streaming_executor::ToolCall;
use crate::tool::{Tool, ToolResult, ToolResultContent, ToolUseContext};

pub type SharedState = Arc<RwLock<AppState>>;

// ── Constants ──

/// Max auto-continue attempts when output is truncated.
pub const MAX_OUTPUT_RECOVERY_LIMIT: u32 = 3;
/// Max chars per tool result before truncation.
const MAX_TOOL_RESULT_CHARS: usize = 50_000;
/// Max total chars for all tool results in one message.
const MAX_TOOL_RESULTS_PER_MESSAGE_CHARS: usize = 200_000;
/// Message injected to continue after output truncation.
const CONTINUE_MESSAGE: &str =
    "Output token limit hit. Resume directly from where you stopped — no apology, no recap, \
     just continue the work. If you were in the middle of a tool call, please re-emit it.";

/// Why a query turn ended.
#[derive(Debug, Clone)]
pub enum StopReason {
    /// Model produced end_turn (normal completion).
    EndTurn,
    /// Tool use — needs follow-up.
    ToolUse,
    /// Max output tokens — may need continuation.
    MaxTokens,
    /// Max turns reached.
    MaxTurnsReached,
    /// Prompt too long and recovery failed.
    PromptTooLong,
    /// Error.
    Error(String),
}

/// Execute a single tool with permission check.
pub async fn execute_tool(
    tool_name: &str,
    tool_input: &serde_json::Value,
    tools: &[Box<dyn Tool>],
    ctx: &ToolUseContext,
    verbose: bool,
) -> ToolResult {
    use crate::permissions;

    let perm = permissions::check_permission(tool_name, ctx.permission_mode);
    let allowed = match perm {
        permissions::PermissionResult::Allowed => true,
        permissions::PermissionResult::Denied(reason) => {
            tracing::warn!("Denied: {reason}");
            false
        }
        permissions::PermissionResult::NeedApproval { description, .. } => {
            permissions::prompt_user_permission(&description)
        }
    };

    if !allowed {
        return ToolResult::error(format!("Permission denied for tool '{tool_name}'"));
    }

    match tools.iter().find(|t| t.name() == tool_name) {
        Some(t) => {
            if verbose {
                tracing::info!("Tool: {tool_name}");
            }
            // Validate input against tool's schema
            let schema = t.input_schema();
            // Validate required fields
            for field in &schema.required {
                if tool_input.get(field).is_none() {
                    return ToolResult::error(format!(
                        "Missing required field '{field}' for tool '{tool_name}'"
                    ));
                }
            }
            match t.execute(tool_input.clone(), ctx).await {
                Ok(r) => r,
                Err(e) => ToolResult::error(format!("Tool error: {e}")),
            }
        }
        None => ToolResult::error(format!("Unknown tool: {tool_name}")),
    }
}

/// Execute multiple tools with concurrency partitioning.
pub async fn execute_tools_parallel(
    tool_uses: &[ToolCall],
    tools: &[Box<dyn Tool>],
    ctx: &ToolUseContext,
    verbose: bool,
) -> Vec<crate::streaming_executor::OrderedToolResult> {
    crate::streaming_executor::execute_streaming(tool_uses, tools, ctx, verbose).await
}

/// Build a user message containing tool results.
pub fn build_tool_result_message(
    results: &[crate::streaming_executor::OrderedToolResult],
) -> Message {
    let blocks: Vec<ContentBlock> = results
        .iter()
        .map(|r| ContentBlock::ToolResult {
            tool_use_id: r.tool_use_id.clone(),
            content: serde_json::Value::String(r.result_text.clone()),
            is_error: r.is_error,
        })
        .collect();
    Message {
        uuid: uuid::Uuid::new_v4(),
        role: Role::User,
        content: blocks,
        timestamp: chrono::Utc::now(),
        model: None,
    }
}

/// Enforce tool result budget: truncate individual results and total per message.
pub fn enforce_tool_result_budget(results: &mut [crate::streaming_executor::OrderedToolResult]) {
    // Per-tool limit
    for r in results.iter_mut() {
        if r.result_text.len() > MAX_TOOL_RESULT_CHARS {
            let truncated = &r.result_text[..MAX_TOOL_RESULT_CHARS];
            // Find last newline to avoid cutting mid-line
            let cut = truncated.rfind('\n').unwrap_or(MAX_TOOL_RESULT_CHARS);
            r.result_text = format!(
                "{}...\n\n[Truncated: result was {} chars, showing first {}]",
                &r.result_text[..cut],
                r.result_text.len(),
                cut
            );
        }
    }

    // Per-message aggregate limit
    let total: usize = results.iter().map(|r| r.result_text.len()).sum();
    if total > MAX_TOOL_RESULTS_PER_MESSAGE_CHARS {
        // Truncate largest results first
        let mut indices: Vec<usize> = (0..results.len()).collect();
        indices.sort_by(|a, b| {
            results[*b]
                .result_text
                .len()
                .cmp(&results[*a].result_text.len())
        });

        let mut current_total = total;
        for &idx in &indices {
            if current_total <= MAX_TOOL_RESULTS_PER_MESSAGE_CHARS {
                break;
            }
            let r = &mut results[idx];
            let target = r.result_text.len() / 2; // halve the largest
            let cut = r.result_text[..target].rfind('\n').unwrap_or(target);
            current_total -= r.result_text.len() - cut;
            r.result_text = format!(
                "{}...\n\n[Truncated to fit budget: was {} chars]",
                &r.result_text[..cut],
                r.result_text.len()
            );
        }
    }
}

/// Build the auto-continue message for max_output_tokens recovery.
pub fn build_continue_message() -> Message {
    Message::user(CONTINUE_MESSAGE)
}

/// Check if a stop reason indicates max_output_tokens truncation.
pub fn is_max_tokens_stop(stop_reason: Option<&str>) -> bool {
    stop_reason.is_some_and(|s| s == "max_tokens")
}

/// Determine fallback model for 529 overload.
/// Returns a smaller model if available.
pub fn get_fallback_model(current_model: &str) -> Option<&'static str> {
    let lower = current_model.to_lowercase();
    if lower.contains("opus")
        || (lower.contains("sonnet") && (lower.contains("4.5") || lower.contains("45")))
    {
        Some("claude-sonnet-4-20250514")
    } else {
        None // No fallback for haiku or base sonnet
    }
}

pub fn extract_result_text(result: &ToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| match c {
            ToolResultContent::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_result_text() {
        let result = ToolResult::text("hello world");
        assert_eq!(extract_result_text(&result), "hello world");
    }

    #[test]
    fn test_build_tool_result_message() {
        let results = vec![crate::streaming_executor::OrderedToolResult {
            tool_use_id: "1".into(),
            tool_name: "Bash".into(),
            result_text: "ok".into(),
            is_error: false,
            duration_ms: 10,
        }];
        let msg = build_tool_result_message(&results);
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content.len(), 1);
    }

    #[test]
    fn test_enforce_tool_result_budget_small() {
        let mut results = vec![crate::streaming_executor::OrderedToolResult {
            tool_use_id: "1".into(),
            tool_name: "Bash".into(),
            result_text: "short".into(),
            is_error: false,
            duration_ms: 10,
        }];
        enforce_tool_result_budget(&mut results);
        assert_eq!(results[0].result_text, "short");
    }

    #[test]
    fn test_enforce_tool_result_budget_large() {
        let mut results = vec![crate::streaming_executor::OrderedToolResult {
            tool_use_id: "1".into(),
            tool_name: "Bash".into(),
            result_text: "x".repeat(100_000),
            is_error: false,
            duration_ms: 10,
        }];
        enforce_tool_result_budget(&mut results);
        assert!(results[0].result_text.len() < 60_000);
        assert!(results[0].result_text.contains("Truncated"));
    }

    #[test]
    fn test_is_max_tokens_stop() {
        assert!(is_max_tokens_stop(Some("max_tokens")));
        assert!(!is_max_tokens_stop(Some("end_turn")));
        assert!(!is_max_tokens_stop(None));
    }

    #[test]
    fn test_get_fallback_model() {
        assert!(get_fallback_model("claude-opus-4-20250514").is_some());
        assert!(get_fallback_model("claude-haiku-3-5-20241022").is_none());
    }

    #[test]
    fn test_build_continue_message() {
        let msg = build_continue_message();
        assert_eq!(msg.role, Role::User);
        assert!(msg.text_content().contains("Resume"));
    }
}
