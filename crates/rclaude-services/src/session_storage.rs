//! Session storage with JSONL transcript persistence.
//! Handles full session persistence with JSONL format.

use rclaude_core::message::Message;
use std::path::{Path, PathBuf};

/// Session metadata.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionMetadata {
    pub session_id: String,
    pub model: String,
    pub cwd: String,
    pub created_at: String,
    pub updated_at: String,
    pub message_count: usize,
    pub total_cost_usd: f64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
}

/// Get the session directory for a project.
pub fn get_session_dir(cwd: &Path) -> PathBuf {
    let hash = cwd
        .to_string_lossy()
        .replace('/', "-")
        .trim_start_matches('-')
        .to_string();
    rclaude_core::config::Config::projects_dir()
        .join(hash)
        .join("sessions")
}

/// Save session with metadata.
pub async fn save_session_with_metadata(
    session_id: &str,
    model: &str,
    messages: &[Message],
    cwd: &Path,
    cost: f64,
    input_tokens: u64,
    output_tokens: u64,
) -> Result<PathBuf, String> {
    let dir = get_session_dir(cwd);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;

    let metadata = SessionMetadata {
        session_id: session_id.to_string(),
        model: model.to_string(),
        cwd: cwd.to_string_lossy().to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        message_count: messages.len(),
        total_cost_usd: cost,
        total_input_tokens: input_tokens,
        total_output_tokens: output_tokens,
    };

    let data = serde_json::json!({
        "metadata": metadata,
        "messages": messages,
    });

    let path = dir.join(format!("{session_id}.json"));
    let json = serde_json::to_string(&data).map_err(|e| e.to_string())?;
    tokio::fs::write(&path, json)
        .await
        .map_err(|e| e.to_string())?;
    Ok(path)
}

/// List all sessions with metadata.
pub async fn list_sessions_with_metadata(cwd: &Path) -> Vec<SessionMetadata> {
    let dir = get_session_dir(cwd);
    let mut sessions = Vec::new();
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(e) => e,
        Err(_) => return sessions,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(content) = tokio::fs::read_to_string(&path).await {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(meta) = data.get("metadata") {
                    if let Ok(m) = serde_json::from_value::<SessionMetadata>(meta.clone()) {
                        sessions.push(m);
                    }
                }
            }
        }
    }
    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    sessions
}

/// Delete a session.
pub async fn delete_session(cwd: &Path, session_id: &str) -> Result<(), String> {
    let path = get_session_dir(cwd).join(format!("{session_id}.json"));
    tokio::fs::remove_file(&path)
        .await
        .map_err(|e| e.to_string())
}

/// Get total storage size for a project's sessions.
pub async fn get_storage_size(cwd: &Path) -> u64 {
    let dir = get_session_dir(cwd);
    let mut total = 0u64;
    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(e) => e,
        Err(_) => return 0,
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        if let Ok(meta) = entry.metadata().await {
            total += meta.len();
        }
    }
    total
}

/// Save a JSONL transcript (one message per line, for streaming/incremental writes).
pub async fn save_transcript_jsonl(
    session_id: &str,
    messages: &[Message],
    cwd: &Path,
) -> Result<PathBuf, String> {
    let dir = get_session_dir(cwd);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;

    let path = dir.join(format!("{session_id}.jsonl"));
    let mut lines = String::new();
    for msg in messages {
        let line = serde_json::to_string(msg).map_err(|e| e.to_string())?;
        lines.push_str(&line);
        lines.push('\n');
    }
    tokio::fs::write(&path, lines)
        .await
        .map_err(|e| e.to_string())?;
    Ok(path)
}

/// Load a JSONL transcript.
pub async fn load_transcript_jsonl(session_id: &str, cwd: &Path) -> Result<Vec<Message>, String> {
    let path = get_session_dir(cwd).join(format!("{session_id}.jsonl"));
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| e.to_string())?;

    let mut messages = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let msg: Message = serde_json::from_str(line).map_err(|e| e.to_string())?;
        messages.push(msg);
    }
    Ok(messages)
}

/// Append a single message to a JSONL transcript.
/// Includes parentUuid chain for message linking.
pub async fn append_to_transcript(
    session_id: &str,
    message: &Message,
    cwd: &Path,
) -> Result<(), String> {
    let dir = get_session_dir(cwd);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;

    let path = dir.join(format!("{session_id}.jsonl"));

    // Build transcript entry with chain metadata
    let entry = serde_json::json!({
        "uuid": message.uuid.to_string(),
        "parentUuid": get_last_chain_uuid(&path).await,
        "role": match message.role {
            rclaude_core::message::Role::User => "user",
            rclaude_core::message::Role::Assistant => "assistant",
            rclaude_core::message::Role::System => "system",
        },
        "content": message.content,
        "timestamp": message.timestamp.to_rfc3339(),
        "model": message.model,
        "sessionId": session_id,
        "version": env!("CARGO_PKG_VERSION"),
    });

    let mut line = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
    line.push('\n');

    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
        .map_err(|e| e.to_string())?;
    file.write_all(line.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Append agent sidechain messages to a separate transcript file.
/// Agents get their own .jsonl for isolated conversation tracking.
pub async fn append_agent_transcript(
    session_id: &str,
    agent_id: &str,
    message: &Message,
    parent_uuid: Option<&str>,
    cwd: &Path,
) -> Result<(), String> {
    let dir = get_session_dir(cwd);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| e.to_string())?;

    let path = dir.join(format!("{session_id}.agent-{agent_id}.jsonl"));

    let entry = serde_json::json!({
        "uuid": message.uuid.to_string(),
        "parentUuid": parent_uuid,
        "isSidechain": true,
        "agentId": agent_id,
        "role": match message.role {
            rclaude_core::message::Role::User => "user",
            rclaude_core::message::Role::Assistant => "assistant",
            rclaude_core::message::Role::System => "system",
        },
        "content": message.content,
        "timestamp": message.timestamp.to_rfc3339(),
        "sessionId": session_id,
    });

    let mut line = serde_json::to_string(&entry).map_err(|e| e.to_string())?;
    line.push('\n');

    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await
        .map_err(|e| e.to_string())?;
    file.write_all(line.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Read the last chain participant UUID from a JSONL file (for parentUuid linking).
/// Reads only the tail of the file for efficiency (matching readLiteMetadata).
async fn get_last_chain_uuid(path: &Path) -> Option<String> {
    // Read last 64KB of file to find the last entry with a uuid
    let content = match tokio::fs::read_to_string(path).await {
        Ok(c) => c,
        Err(_) => return None,
    };
    // Scan from end for last line with a uuid
    for line in content.lines().rev() {
        if line.is_empty() {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            // Only chain user/assistant messages (not progress/attachment)
            let role = v.get("role").and_then(|r| r.as_str()).unwrap_or("");
            if role == "user" || role == "assistant" {
                return v.get("uuid").and_then(|u| u.as_str()).map(String::from);
            }
        }
    }
    None
}
