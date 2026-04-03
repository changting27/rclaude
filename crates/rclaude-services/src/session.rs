//! Session persistence: save and load conversation state to disk.

use rclaude_core::config::Config;
use rclaude_core::error::Result;
use rclaude_core::message::Message;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Persisted session data.
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionData {
    pub session_id: String,
    pub model: String,
    pub messages: Vec<Message>,
    pub cwd: String,
    pub created_at: String,
    pub updated_at: String,
    /// Q09: Session title (auto-generated or user-set).
    #[serde(default)]
    pub title: Option<String>,
    /// Q09: User-defined tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Q09: Git branch at session start.
    #[serde(default)]
    pub branch: Option<String>,
}

/// Get the sessions directory (~/.claude/projects/<project-hash>/sessions/).
/// Sanitizes path: replace non-alphanumeric with '-', truncate+hash if too long.
fn sessions_dir(cwd: &std::path::Path) -> PathBuf {
    Config::projects_dir()
        .join(sanitize_path(&cwd.to_string_lossy()))
        .join("sessions")
}

/// Sanitize path: replace non-alphanumeric chars with '-', truncate+hash if too long.
fn sanitize_path(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    const MAX_LEN: usize = 80;
    if sanitized.len() <= MAX_LEN {
        sanitized
    } else {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut hasher);
        let hash = hasher.finish();
        format!("{}-{:x}", &sanitized[..MAX_LEN], hash)
    }
}

/// Save a session to disk.
pub async fn save_session(
    session_id: &str,
    model: &str,
    messages: &[Message],
    cwd: &std::path::Path,
) -> Result<PathBuf> {
    let dir = sessions_dir(cwd);
    tokio::fs::create_dir_all(&dir).await?;

    let data = SessionData {
        session_id: session_id.to_string(),
        model: model.to_string(),
        messages: messages.to_vec(),
        cwd: cwd.to_string_lossy().to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        title: None,
        tags: Vec::new(),
        branch: None,
    };

    let file_path = dir.join(format!("{session_id}.json"));
    let json = serde_json::to_string(&data)?;
    tokio::fs::write(&file_path, json).await?;

    Ok(file_path)
}

/// Load the most recent session for the given cwd.
pub async fn load_latest_session(cwd: &std::path::Path) -> Result<Option<SessionData>> {
    let dir = sessions_dir(cwd);
    if !dir.exists() {
        return Ok(None);
    }

    let mut entries = tokio::fs::read_dir(&dir).await?;
    let mut latest: Option<(std::time::SystemTime, PathBuf)> = None;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            if let Ok(meta) = entry.metadata().await {
                if let Ok(modified) = meta.modified() {
                    match &latest {
                        None => latest = Some((modified, path)),
                        Some((prev_time, _)) if modified > *prev_time => {
                            latest = Some((modified, path));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    if let Some((_, path)) = latest {
        let content = tokio::fs::read_to_string(&path).await?;
        let data: SessionData = serde_json::from_str(&content)?;
        Ok(Some(data))
    } else {
        Ok(None)
    }
}

/// Load a specific session by ID.
pub async fn load_session(session_id: &str, cwd: &std::path::Path) -> Result<Option<SessionData>> {
    let path = sessions_dir(cwd).join(format!("{session_id}.json"));
    if !path.exists() {
        return Ok(None);
    }
    let content = tokio::fs::read_to_string(&path).await?;
    let data: SessionData = serde_json::from_str(&content)?;
    Ok(Some(data))
}

/// List all sessions for the given cwd, sorted by most recent first.
pub async fn list_sessions(cwd: &std::path::Path) -> Result<Vec<SessionSummary>> {
    let dir = sessions_dir(cwd);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    let mut entries = tokio::fs::read_dir(&dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(content) = tokio::fs::read_to_string(&path).await {
            if let Ok(data) = serde_json::from_str::<SessionData>(&content) {
                sessions.push(SessionSummary {
                    session_id: data.session_id,
                    model: data.model,
                    message_count: data.messages.len(),
                    updated_at: data.updated_at,
                });
            }
        }
    }

    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(sessions)
}

/// Search sessions by keyword in message content.
/// Searches text content, tool inputs, and tool results.
pub async fn search_sessions(cwd: &std::path::Path, query: &str) -> Result<Vec<SessionSummary>> {
    let all = list_sessions(cwd).await?;
    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    for summary in all {
        // Try JSONL transcript first (more detailed)
        let jsonl_path = crate::session_storage::get_session_dir(cwd)
            .join(format!("{}.jsonl", summary.session_id));
        let found = if jsonl_path.exists() {
            search_transcript_file(&jsonl_path, &query_lower).await
        } else {
            // Fallback to JSON session file
            let json_path = sessions_dir(cwd).join(format!("{}.json", summary.session_id));
            if let Ok(content) = tokio::fs::read_to_string(&json_path).await {
                content.to_lowercase().contains(&query_lower)
            } else {
                false
            }
        };
        if found {
            matches.push(summary);
        }
    }
    Ok(matches)
}

/// Search a JSONL transcript file for a query string.
/// Searches text blocks, tool_use inputs, and tool_result content.
async fn search_transcript_file(path: &std::path::Path, query_lower: &str) -> bool {
    let content = match tokio::fs::read_to_string(path).await {
        Ok(c) => c,
        Err(_) => return false,
    };
    for line in content.lines() {
        if line.is_empty() {
            continue;
        }
        let v: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        // Search text content
        if let Some(content_arr) = v.get("content").and_then(|c| c.as_array()) {
            for block in content_arr {
                // Text blocks
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    if text.to_lowercase().contains(query_lower) {
                        return true;
                    }
                }
                // Tool use: search input fields (command, pattern, file_path, prompt, etc.)
                if let Some(input) = block.get("input") {
                    for key in &[
                        "command",
                        "pattern",
                        "file_path",
                        "path",
                        "prompt",
                        "description",
                        "query",
                        "url",
                    ] {
                        if let Some(val) = input.get(*key).and_then(|v| v.as_str()) {
                            if val.to_lowercase().contains(query_lower) {
                                return true;
                            }
                        }
                    }
                }
                // Tool result content
                if let Some(result_content) = block.get("content").and_then(|c| c.as_str()) {
                    if result_content.to_lowercase().contains(query_lower) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Brief session info for listing.
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub model: String,
    pub message_count: usize,
    pub updated_at: String,
}

/// Q09: Acquire a session lock (detect concurrent sessions).
/// Returns Ok(lock_path) if acquired, Err if another session is active.
pub async fn acquire_session_lock(
    session_id: &str,
    cwd: &std::path::Path,
) -> std::result::Result<PathBuf, String> {
    let lock_path = sessions_dir(cwd).join(".lock");
    tokio::fs::create_dir_all(lock_path.parent().unwrap())
        .await
        .ok();

    // Check existing lock
    if lock_path.exists() {
        if let Ok(content) = tokio::fs::read_to_string(&lock_path).await {
            let parts: Vec<&str> = content.split(':').collect();
            if let Some(pid_str) = parts.first() {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    // Check if process is still alive
                    let alive = std::path::Path::new(&format!("/proc/{pid}")).exists();
                    if alive {
                        let other_id = parts.get(1).unwrap_or(&"unknown");
                        return Err(format!(
                            "Another session ({other_id}) is active (PID {pid})"
                        ));
                    }
                }
            }
        }
    }

    // Write our lock
    let content = format!("{}:{}", std::process::id(), session_id);
    tokio::fs::write(&lock_path, content)
        .await
        .map_err(|e| format!("Failed to write lock: {e}"))?;
    Ok(lock_path)
}

/// Q09: Release a session lock.
pub async fn release_session_lock(cwd: &std::path::Path) {
    let lock_path = sessions_dir(cwd).join(".lock");
    let _ = tokio::fs::remove_file(&lock_path).await;
}

/// Q09: Generate a session title from the first user message.
pub fn generate_session_title(messages: &[Message]) -> Option<String> {
    messages
        .iter()
        .find(|m| m.role == rclaude_core::message::Role::User)
        .map(|m| {
            let text = m.text_content();
            if text.len() > 60 {
                format!("{}...", &text[..57])
            } else {
                text
            }
        })
}
