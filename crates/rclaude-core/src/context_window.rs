//! Context window management: estimate token count and auto-compact.
//!
//! Compact behavior:
//! - Truncate large tool results before dropping messages
//! - Insert a summary marker so the model knows context was compacted
//! - Preserve the first user message + last N turns

use crate::message::{ContentBlock, Message, Role};

/// Rough token estimation (4 chars ≈ 1 token for English).
const CHARS_PER_TOKEN: usize = 4;

/// Default context window size.
const DEFAULT_CONTEXT_WINDOW: usize = 200_000;

/// Threshold to trigger auto-compact (80% of window).
const COMPACT_THRESHOLD_RATIO: f64 = 0.8;

/// Max chars to keep per tool result during compaction.
const TOOL_RESULT_COMPACT_LIMIT: usize = 2000;

/// Estimate token count for a single message.
pub fn estimate_message_tokens(msg: &Message) -> usize {
    let text_len: usize = msg
        .content
        .iter()
        .map(|block| match block {
            ContentBlock::Text { text } => text.len(),
            ContentBlock::ToolUse { input, .. } => input.to_string().len(),
            ContentBlock::ToolResult { content, .. } => content.to_string().len(),
            ContentBlock::Thinking { thinking } => thinking.len(),
            ContentBlock::Image { .. } => 1000,
        })
        .sum();

    text_len / CHARS_PER_TOKEN
}

/// Estimate total tokens in conversation.
pub fn estimate_conversation_tokens(messages: &[Message]) -> usize {
    messages.iter().map(estimate_message_tokens).sum()
}

/// Check if auto-compact should be triggered.
pub fn should_auto_compact(messages: &[Message], context_window: Option<usize>) -> bool {
    let window = context_window.unwrap_or(DEFAULT_CONTEXT_WINDOW);
    let tokens = estimate_conversation_tokens(messages);
    tokens as f64 > window as f64 * COMPACT_THRESHOLD_RATIO
}

/// Smart compact: truncate tool results first, then drop old messages.
/// Preserves the first user message + last `keep_last` pairs.
/// Inserts a summary marker so the model knows context was compacted.
pub fn compact_messages(messages: &[Message], keep_last: usize) -> Vec<Message> {
    if messages.len() <= keep_last * 2 {
        return messages.to_vec();
    }

    // Phase 1: Truncate large tool results in older messages
    let mut msgs: Vec<Message> = messages.to_vec();
    let cutoff = msgs.len().saturating_sub(keep_last * 2);
    for msg in msgs[..cutoff].iter_mut() {
        for block in msg.content.iter_mut() {
            if let ContentBlock::ToolResult { content, .. } = block {
                let s = content.to_string();
                if s.len() > TOOL_RESULT_COMPACT_LIMIT {
                    let truncated = format!(
                        "{}... [truncated, was {} chars]",
                        &s[..TOOL_RESULT_COMPACT_LIMIT],
                        s.len()
                    );
                    *content = serde_json::Value::String(truncated);
                }
            }
        }
    }

    // Check if truncation alone was enough
    if !should_auto_compact(&msgs, None) {
        return msgs;
    }

    // Phase 2: Drop old messages, keep structure
    let mut result = Vec::new();

    // Keep system messages from the start
    for msg in msgs.iter() {
        if msg.role == Role::System {
            result.push(msg.clone());
        } else {
            break;
        }
    }

    // Keep the first user message for context
    if let Some(first_user) = msgs.iter().find(|m| m.role == Role::User) {
        if !result.iter().any(|m| m.uuid == first_user.uuid) {
            result.push(first_user.clone());
        }
    }

    // Insert compaction marker
    result.push(Message::assistant(vec![ContentBlock::Text {
        text: "[Earlier conversation history was summarized to save context space. \
               The conversation continues below.]"
            .to_string(),
    }]));

    // Keep the last N messages
    let start = msgs.len().saturating_sub(keep_last * 2);
    result.extend_from_slice(&msgs[start..]);

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        let msg = Message::user("Hello, how are you doing today?");
        let tokens = estimate_message_tokens(&msg);
        assert!(tokens > 0 && tokens < 20);
    }

    #[test]
    fn test_compact_preserves_recent() {
        // Create messages large enough to trigger compaction
        let msgs: Vec<Message> = (0..20)
            .map(|i| Message::user("x".repeat(50000) + &format!(" Message {i}")))
            .collect();
        let compacted = compact_messages(&msgs, 4);
        assert!(compacted.len() < msgs.len());
        assert!(compacted
            .last()
            .unwrap()
            .text_content()
            .contains("Message 19"));
    }

    #[test]
    fn test_compact_inserts_summary_marker() {
        let msgs: Vec<Message> = (0..20)
            .map(|i| Message::user("x".repeat(50000) + &format!(" Message {i}")))
            .collect();
        let compacted = compact_messages(&msgs, 4);
        let has_marker = compacted
            .iter()
            .any(|m| m.text_content().contains("summarized"));
        assert!(has_marker, "Should contain summary marker");
    }

    #[test]
    fn test_compact_preserves_first_user_message() {
        let msgs: Vec<Message> = (0..20)
            .map(|i| Message::user("x".repeat(50000) + &format!(" Message {i}")))
            .collect();
        let compacted = compact_messages(&msgs, 4);
        assert!(compacted
            .iter()
            .any(|m| m.text_content().contains("Message 0")));
    }

    #[test]
    fn test_compact_noop_when_small() {
        let msgs: Vec<Message> = (0..5)
            .map(|i| Message::user(format!("Short message {i}")))
            .collect();
        let compacted = compact_messages(&msgs, 4);
        assert_eq!(compacted.len(), msgs.len());
    }

    #[test]
    fn test_compact_truncates_tool_results() {
        let big_content = "x".repeat(5000);
        let mut msgs = vec![
            Message::user("hello"),
            Message::assistant(vec![ContentBlock::ToolUse {
                id: "1".into(),
                name: "Read".into(),
                input: serde_json::json!({}),
            }]),
            Message {
                uuid: uuid::Uuid::new_v4(),
                role: Role::User,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "1".into(),
                    content: serde_json::Value::String(big_content),
                    is_error: false,
                }],
                timestamp: chrono::Utc::now(),
                model: None,
            },
        ];
        // Add enough messages to trigger compaction
        for i in 0..20 {
            msgs.push(Message::user(format!("msg {i}")));
        }
        let compacted = compact_messages(&msgs, 4);
        // The tool result in the old part should be truncated
        for m in &compacted {
            for block in &m.content {
                if let ContentBlock::ToolResult { content, .. } = block {
                    assert!(content.to_string().len() < 3000);
                }
            }
        }
    }
}
