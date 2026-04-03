//! Message processing utilities.

use crate::message::{ContentBlock, Message, Role};

/// Validate that messages alternate correctly (user/assistant).
pub fn validate_message_order(messages: &[Message]) -> Result<(), String> {
    let mut last_role = None;
    for (i, msg) in messages.iter().enumerate() {
        if msg.role == Role::System {
            continue;
        }
        if let Some(prev) = last_role {
            if prev == msg.role && msg.role != Role::User {
                return Err(format!(
                    "Message {i}: consecutive {} messages",
                    if msg.role == Role::User {
                        "user"
                    } else {
                        "assistant"
                    }
                ));
            }
        }
        last_role = Some(msg.role);
    }
    Ok(())
}

/// Collapse consecutive tool results into a summary to save context.
pub fn collapse_tool_results(messages: &mut [Message], max_result_chars: usize) {
    for msg in messages.iter_mut() {
        if msg.role != Role::User {
            continue;
        }
        for block in msg.content.iter_mut() {
            if let ContentBlock::ToolResult { content, .. } = block {
                let s = content.to_string();
                if s.len() > max_result_chars {
                    let truncated = format!(
                        "{}... [truncated from {} chars]",
                        &s[..max_result_chars.min(s.len())],
                        s.len()
                    );
                    *content = serde_json::Value::String(truncated);
                }
            }
        }
    }
}

/// Count total text length across all messages.
pub fn total_text_length(messages: &[Message]) -> usize {
    messages
        .iter()
        .map(|m| {
            m.content
                .iter()
                .map(|b| match b {
                    ContentBlock::Text { text } => text.len(),
                    ContentBlock::ToolUse { input, .. } => input.to_string().len(),
                    ContentBlock::ToolResult { content, .. } => content.to_string().len(),
                    ContentBlock::Thinking { thinking } => thinking.len(),
                    ContentBlock::Image { .. } => 0,
                })
                .sum::<usize>()
        })
        .sum()
}

/// Extract the last assistant text response.
pub fn last_assistant_text(messages: &[Message]) -> Option<&str> {
    messages
        .iter()
        .rev()
        .find(|m| m.role == Role::Assistant)
        .and_then(|m| {
            m.content.iter().find_map(|b| match b {
                ContentBlock::Text { text } if !text.is_empty() => Some(text.as_str()),
                _ => None,
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_order_ok() {
        let msgs = vec![
            Message::user("hi"),
            Message::assistant(vec![ContentBlock::Text {
                text: "hello".into(),
            }]),
            Message::user("bye"),
        ];
        assert!(validate_message_order(&msgs).is_ok());
    }

    #[test]
    fn test_total_text_length() {
        let msgs = vec![
            Message::user("hello"),
            Message::assistant(vec![ContentBlock::Text {
                text: "world".into(),
            }]),
        ];
        assert_eq!(total_text_length(&msgs), 10);
    }

    #[test]
    fn test_last_assistant_text() {
        let msgs = vec![
            Message::user("hi"),
            Message::assistant(vec![ContentBlock::Text {
                text: "response".into(),
            }]),
        ];
        assert_eq!(last_assistant_text(&msgs), Some("response"));
    }
}
