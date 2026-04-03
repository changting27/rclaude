//! Conversation recovery for interrupted sessions.
//! Handles loading and restoring conversation state from disk.

use rclaude_core::message::Message;
use std::path::Path;

/// Load conversation messages from a session file.
pub async fn load_conversation(session_path: &Path) -> Result<Vec<Message>, String> {
    let content = tokio::fs::read_to_string(session_path)
        .await
        .map_err(|e| format!("Failed to read session: {e}"))?;

    let data: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse session: {e}"))?;

    let messages = data
        .get("messages")
        .and_then(|v| v.as_array())
        .ok_or("No messages in session")?;

    let mut result = Vec::new();
    for msg in messages {
        if let Ok(m) = serde_json::from_value::<Message>(msg.clone()) {
            result.push(m);
        }
    }
    Ok(result)
}

/// Check if a session can be resumed (has valid messages).
pub async fn can_resume(session_path: &Path) -> bool {
    load_conversation(session_path)
        .await
        .is_ok_and(|msgs| !msgs.is_empty())
}

/// Find the most recent session file for a project.
pub async fn find_latest_session(cwd: &Path) -> Option<std::path::PathBuf> {
    let sessions_dir = rclaude_core::config::Config::projects_dir()
        .join(project_hash(cwd))
        .join("sessions");

    if !sessions_dir.exists() {
        return None;
    }

    let mut latest: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
    let mut entries = tokio::fs::read_dir(&sessions_dir).await.ok()?;

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(meta) = entry.metadata().await {
            if let Ok(modified) = meta.modified() {
                if latest.as_ref().is_none_or(|(t, _)| modified > *t) {
                    latest = Some((modified, path));
                }
            }
        }
    }
    latest.map(|(_, p)| p)
}

fn project_hash(cwd: &Path) -> String {
    cwd.to_string_lossy()
        .replace('/', "-")
        .trim_start_matches('-')
        .to_string()
}

/// Q13: Detect incomplete tool calls (tool_use without matching tool_result).
/// Returns the IDs of orphaned tool_use blocks.
pub fn find_incomplete_tool_calls(messages: &[Message]) -> Vec<String> {
    use rclaude_core::message::ContentBlock;
    use std::collections::HashSet;

    let mut tool_use_ids: HashSet<String> = HashSet::new();
    let mut tool_result_ids: HashSet<String> = HashSet::new();

    for msg in messages {
        for block in &msg.content {
            match block {
                ContentBlock::ToolUse { id, .. } => {
                    tool_use_ids.insert(id.clone());
                }
                ContentBlock::ToolResult { tool_use_id, .. } => {
                    tool_result_ids.insert(tool_use_id.clone());
                }
                _ => {}
            }
        }
    }

    tool_use_ids.difference(&tool_result_ids).cloned().collect()
}

/// Q13: Fix incomplete tool calls by adding error results for orphaned tool_use blocks.
pub fn fix_incomplete_tool_calls(messages: &mut Vec<Message>) {
    use rclaude_core::message::{ContentBlock, Role};

    let orphans = find_incomplete_tool_calls(messages);
    if orphans.is_empty() {
        return;
    }

    // Add tool_result blocks for orphaned tool_use
    let result_blocks: Vec<ContentBlock> = orphans
        .iter()
        .map(|id| ContentBlock::ToolResult {
            tool_use_id: id.clone(),
            content: serde_json::Value::String(
                "(Session was interrupted before this tool completed)".into(),
            ),
            is_error: true,
        })
        .collect();

    messages.push(Message {
        uuid: uuid::Uuid::new_v4(),
        role: Role::User,
        content: result_blocks,
        timestamp: chrono::Utc::now(),
        model: None,
    });
}

/// Q13: Try to recover from a JSONL transcript file.
pub async fn recover_from_transcript(session_id: &str, cwd: &Path) -> Result<Vec<Message>, String> {
    let transcript_path = rclaude_core::config::Config::projects_dir()
        .join(project_hash(cwd))
        .join("sessions")
        .join(format!("{session_id}.jsonl"));

    if !transcript_path.exists() {
        return Err("No transcript file found".into());
    }

    crate::session_storage::load_transcript_jsonl(session_id, cwd).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use rclaude_core::message::{ContentBlock, Role};

    #[test]
    fn test_find_incomplete_tool_calls() {
        let messages = vec![
            Message::assistant(vec![ContentBlock::ToolUse {
                id: "1".into(),
                name: "Bash".into(),
                input: serde_json::json!({}),
            }]),
            // No tool_result for "1"
        ];
        let orphans = find_incomplete_tool_calls(&messages);
        assert_eq!(orphans.len(), 1);
        assert!(orphans.contains(&"1".to_string()));
    }

    #[test]
    fn test_find_complete_tool_calls() {
        let messages = vec![
            Message::assistant(vec![ContentBlock::ToolUse {
                id: "1".into(),
                name: "Bash".into(),
                input: serde_json::json!({}),
            }]),
            Message {
                uuid: uuid::Uuid::new_v4(),
                role: Role::User,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "1".into(),
                    content: serde_json::Value::String("ok".into()),
                    is_error: false,
                }],
                timestamp: chrono::Utc::now(),
                model: None,
            },
        ];
        let orphans = find_incomplete_tool_calls(&messages);
        assert!(orphans.is_empty());
    }

    #[test]
    fn test_fix_incomplete_tool_calls() {
        let mut messages = vec![Message::assistant(vec![ContentBlock::ToolUse {
            id: "1".into(),
            name: "Bash".into(),
            input: serde_json::json!({}),
        }])];
        fix_incomplete_tool_calls(&mut messages);
        assert_eq!(messages.len(), 2);
        assert!(messages[1].content.iter().any(|b| matches!(
            b,
            ContentBlock::ToolResult {
                tool_use_id,
                is_error: true,
                ..
            } if tool_use_id == "1"
        )));
    }
}
