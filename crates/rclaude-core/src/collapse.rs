//! Message collapsing for read/search tool results.

use crate::message::{ContentBlock, Message, Role};

/// Collapse consecutive read/search tool results into summaries.
/// Reduces context window usage by summarizing repetitive tool outputs.
pub fn collapse_read_search_groups(messages: &mut [Message]) {
    let mut i = 0;
    while i < messages.len() {
        if messages[i].role != Role::User {
            i += 1;
            continue;
        }

        // Check if this is a tool result message with read/search results
        let tool_results: Vec<usize> = messages[i]
            .content
            .iter()
            .enumerate()
            .filter_map(|(idx, block)| {
                if let ContentBlock::ToolResult {
                    content, is_error, ..
                } = block
                {
                    if !is_error {
                        let text = content.to_string();
                        if text.len() > 5000 {
                            return Some(idx);
                        }
                    }
                }
                None
            })
            .collect();

        // Truncate large tool results
        for idx in tool_results.iter().rev() {
            if let ContentBlock::ToolResult { content, .. } = &mut messages[i].content[*idx] {
                let text = content.to_string();
                if text.len() > 5000 {
                    let truncated =
                        format!("{}... [truncated from {} chars]", &text[..2000], text.len());
                    *content = serde_json::Value::String(truncated);
                }
            }
        }
        i += 1;
    }
}

/// Get a summary of recent tool activities.
pub fn summarize_recent_activities(messages: &[Message], max_items: usize) -> Vec<String> {
    let mut activities = Vec::new();
    for msg in messages.iter().rev() {
        if activities.len() >= max_items {
            break;
        }
        for block in &msg.content {
            if let ContentBlock::ToolUse { name, input, .. } = block {
                let detail = input
                    .get("file_path")
                    .or(input.get("command"))
                    .or(input.get("pattern"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let preview = if detail.len() > 50 {
                    &detail[..50]
                } else {
                    detail
                };
                activities.push(format!("{name}: {preview}"));
            }
        }
    }
    activities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summarize_activities() {
        let msgs = vec![Message::assistant(vec![ContentBlock::ToolUse {
            id: "1".into(),
            name: "Read".into(),
            input: serde_json::json!({"file_path": "src/main.rs"}),
        }])];
        let activities = summarize_recent_activities(&msgs, 5);
        assert_eq!(activities.len(), 1);
        assert!(activities[0].contains("Read"));
    }
}
