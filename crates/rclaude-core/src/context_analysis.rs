//! Context analysis for conversation state evaluation.
//! Analyzes conversation context for optimization.

use crate::message::{ContentBlock, Message, Role};

/// Context analysis result.
#[derive(Debug)]
pub struct ContextAnalysis {
    pub total_tokens_estimate: usize,
    pub system_tokens: usize,
    pub user_tokens: usize,
    pub assistant_tokens: usize,
    pub tool_result_tokens: usize,
    pub message_count: usize,
    pub tool_use_count: usize,
    pub largest_message_tokens: usize,
    pub context_utilization: f64, // 0.0 - 1.0
}

/// Analyze the current conversation context.
pub fn analyze_context(messages: &[Message], context_window: usize) -> ContextAnalysis {
    let mut system_tokens = 0;
    let mut user_tokens = 0;
    let mut assistant_tokens = 0;
    let mut tool_result_tokens = 0;
    let mut tool_use_count = 0;
    let mut largest = 0;

    for msg in messages {
        let msg_tokens = crate::context_window::estimate_message_tokens(msg);
        largest = largest.max(msg_tokens);

        match msg.role {
            Role::System => system_tokens += msg_tokens,
            Role::User => {
                // Check if it's a tool result message
                let has_tool_result = msg
                    .content
                    .iter()
                    .any(|b| matches!(b, ContentBlock::ToolResult { .. }));
                if has_tool_result {
                    tool_result_tokens += msg_tokens;
                } else {
                    user_tokens += msg_tokens;
                }
            }
            Role::Assistant => {
                assistant_tokens += msg_tokens;
                tool_use_count += msg
                    .content
                    .iter()
                    .filter(|b| matches!(b, ContentBlock::ToolUse { .. }))
                    .count();
            }
        }
    }

    let total = system_tokens + user_tokens + assistant_tokens + tool_result_tokens;

    ContextAnalysis {
        total_tokens_estimate: total,
        system_tokens,
        user_tokens,
        assistant_tokens,
        tool_result_tokens,
        message_count: messages.len(),
        tool_use_count,
        largest_message_tokens: largest,
        context_utilization: total as f64 / context_window as f64,
    }
}

/// Format context analysis for display.
pub fn format_analysis(analysis: &ContextAnalysis) -> String {
    format!(
        "Context: {:.1}% ({} / {} tokens)\n\
         Messages: {} ({} tool calls)\n\
         Breakdown: system={} user={} assistant={} tool_results={}\n\
         Largest message: {} tokens",
        analysis.context_utilization * 100.0,
        analysis.total_tokens_estimate,
        200_000,
        analysis.message_count,
        analysis.tool_use_count,
        analysis.system_tokens,
        analysis.user_tokens,
        analysis.assistant_tokens,
        analysis.tool_result_tokens,
        analysis.largest_message_tokens,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analyze_empty() {
        let analysis = analyze_context(&[], 200_000);
        assert_eq!(analysis.total_tokens_estimate, 0);
        assert_eq!(analysis.context_utilization, 0.0);
    }

    #[test]
    fn test_analyze_messages() {
        let msgs = vec![
            Message::user("Hello, help me with this code"),
            Message::assistant(vec![ContentBlock::Text {
                text: "Sure, I can help!".into(),
            }]),
        ];
        let analysis = analyze_context(&msgs, 200_000);
        assert!(analysis.total_tokens_estimate > 0);
        assert_eq!(analysis.message_count, 2);
    }
}
