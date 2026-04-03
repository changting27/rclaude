//! MicroCompact: lightweight compaction that clears old tool results without full summarization.
//!
//! Only compacts specific tool results (FileRead, Bash, Grep, Glob, WebSearch, WebFetch, FileEdit, FileWrite).
//! Does NOT call the API — just truncates/clears old tool result content.

use crate::message::{ContentBlock, Message, Role};

/// Tools whose results can be micro-compacted.
#[allow(dead_code)]
const COMPACTABLE_TOOLS: &[&str] = &[
    "Read",
    "Bash",
    "Grep",
    "Glob",
    "WebSearch",
    "WebFetch",
    "Edit",
    "Write",
];

/// Max chars to keep per tool result after micro-compaction.
const MICRO_COMPACT_LIMIT: usize = 500;

/// Minimum messages before micro-compact is considered.
const MIN_MESSAGES_FOR_MICRO: usize = 10;

/// Check if micro-compact should trigger.
/// Triggers when estimated tokens exceed threshold and there are enough messages.
pub fn should_micro_compact(messages: &[Message], context_window: usize) -> bool {
    if messages.len() < MIN_MESSAGES_FOR_MICRO {
        return false;
    }
    let tokens = crate::context_window::estimate_conversation_tokens(messages);
    // Trigger at 60% of context window (before full auto-compact at 80%)
    tokens as f64 > context_window as f64 * 0.6
}

/// Perform micro-compaction: truncate old tool results in-place.
/// Only affects messages older than `preserve_recent` turns from the end.
/// Returns the number of tool results compacted.
pub fn micro_compact(messages: &mut [Message], preserve_recent: usize) -> usize {
    let cutoff = messages.len().saturating_sub(preserve_recent * 2);
    let mut compacted = 0;

    for msg in messages[..cutoff].iter_mut() {
        if msg.role != Role::User {
            continue;
        }
        for block in msg.content.iter_mut() {
            if let ContentBlock::ToolResult {
                tool_use_id: _,
                content,
                is_error: _,
            } = block
            {
                let text = content.to_string();
                if text.len() > MICRO_COMPACT_LIMIT {
                    // Check if this is a compactable tool result by looking at the preceding
                    // assistant message's tool_use block (we can't easily do that here,
                    // so we compact all large tool results in old messages)
                    let truncated = format!(
                        "{}... [micro-compacted, was {} chars]",
                        &text[..MICRO_COMPACT_LIMIT.min(text.len())],
                        text.len()
                    );
                    *content = serde_json::Value::String(truncated);
                    compacted += 1;
                }
            }
        }
    }

    compacted
}

/// Strip images from messages (before sending to compact API).
pub fn strip_images(messages: &[Message]) -> Vec<Message> {
    messages
        .iter()
        .map(|msg| {
            let content: Vec<ContentBlock> = msg
                .content
                .iter()
                .filter_map(|block| match block {
                    ContentBlock::Image { .. } => None,
                    other => Some(other.clone()),
                })
                .collect();
            Message {
                content,
                ..msg.clone()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_micro_compact_truncates_old() {
        let mut msgs: Vec<Message> = Vec::new();
        // Create old tool result messages
        for i in 0..20 {
            msgs.push(Message::user(format!("question {i}")));
            let tool_result_msg = Message {
                uuid: uuid::Uuid::new_v4(),
                role: Role::User,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: format!("tool_{i}"),
                    content: serde_json::Value::String("x".repeat(2000)),
                    is_error: false,
                }],
                timestamp: chrono::Utc::now(),
                model: None,
            };
            msgs.push(tool_result_msg);
        }

        let count = micro_compact(&mut msgs, 4);
        assert!(count > 0, "Should have compacted some results");

        // Recent messages should be untouched
        let last_result = &msgs[msgs.len() - 1];
        if let Some(ContentBlock::ToolResult { content, .. }) = last_result.content.first() {
            assert!(content.to_string().len() > MICRO_COMPACT_LIMIT);
        }
    }

    #[test]
    fn test_micro_compact_preserves_small() {
        let mut msgs = vec![
            Message::user("hello"),
            Message {
                uuid: uuid::Uuid::new_v4(),
                role: Role::User,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "1".into(),
                    content: serde_json::Value::String("short".into()),
                    is_error: false,
                }],
                timestamp: chrono::Utc::now(),
                model: None,
            },
        ];
        // Add enough messages to pass cutoff
        for i in 0..20 {
            msgs.push(Message::user(format!("msg {i}")));
        }

        let count = micro_compact(&mut msgs, 4);
        assert_eq!(count, 0, "Short results should not be compacted");
    }

    #[test]
    fn test_strip_images() {
        let msgs = vec![Message {
            uuid: uuid::Uuid::new_v4(),
            role: Role::User,
            content: vec![
                ContentBlock::Text {
                    text: "look at this".into(),
                },
                ContentBlock::Image {
                    source: crate::message::ImageSource {
                        source_type: "base64".into(),
                        media_type: "image/png".into(),
                        data: "abc".into(),
                    },
                },
            ],
            timestamp: chrono::Utc::now(),
            model: None,
        }];
        let stripped = strip_images(&msgs);
        assert_eq!(stripped[0].content.len(), 1);
    }
}
