//! Session memory service matching services/SessionMemory/.
//! Extracts and persists key information from conversations.

use rclaude_core::message::{ContentBlock, Message, Role};
use std::path::{Path, PathBuf};

/// Memory entry extracted from a conversation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    pub source: String, // "auto" or "manual"
    pub timestamp: String,
}

/// Session memory file path.
pub fn memory_file_path(cwd: &Path) -> PathBuf {
    cwd.join(".claude/memory.md")
}

/// Check if memory extraction should run (matching shouldExtractMemory).
pub fn should_extract_memory(messages: &[Message]) -> bool {
    // Extract after every 10 assistant messages
    let assistant_count = messages
        .iter()
        .filter(|m| m.role == Role::Assistant)
        .count();
    assistant_count > 0 && assistant_count % 10 == 0
}

/// Extract key facts from recent messages for memory.
pub fn extract_facts_from_messages(messages: &[Message]) -> Vec<String> {
    let mut facts = Vec::new();
    // Look at the last 10 messages for key information
    for msg in messages.iter().rev().take(10) {
        for block in &msg.content {
            if let ContentBlock::Text { text } = block {
                // Extract file paths mentioned
                for word in text.split_whitespace() {
                    if (word.contains('/') || word.contains('.'))
                        && !word.starts_with("http")
                        && word.len() > 3
                        && word.len() < 200
                        && (word.ends_with(".rs")
                            || word.ends_with(".ts")
                            || word.ends_with(".py")
                            || word.ends_with(".go")
                            || word.ends_with(".js"))
                    {
                        facts.push(format!("File: {word}"));
                    }
                }
            }
        }
    }
    facts.dedup();
    facts.truncate(20);
    facts
}

/// Save memory entries to the memory file.
pub async fn save_memory(cwd: &Path, entries: &[MemoryEntry]) -> Result<(), String> {
    let path = memory_file_path(cwd);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| e.to_string())?;
    }

    let mut content = String::from("# Session Memory\n\n");
    for entry in entries {
        content.push_str(&format!(
            "## {}\n{}\n_Source: {}, {}_\n\n",
            entry.key, entry.value, entry.source, entry.timestamp
        ));
    }

    tokio::fs::write(&path, content)
        .await
        .map_err(|e| e.to_string())
}

/// Load memory entries from the memory file.
pub async fn load_memory(cwd: &Path) -> Vec<MemoryEntry> {
    let path = memory_file_path(cwd);
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut entries = Vec::new();
    let mut current_key = String::new();
    let mut current_value = String::new();

    for line in content.lines() {
        if line.starts_with("## ") {
            if !current_key.is_empty() {
                entries.push(MemoryEntry {
                    key: current_key.clone(),
                    value: current_value.trim().to_string(),
                    source: "loaded".into(),
                    timestamp: String::new(),
                });
            }
            current_key = line.strip_prefix("## ").unwrap_or(line).to_string();
            current_value.clear();
        } else if !line.starts_with('_') && !current_key.is_empty() {
            current_value.push_str(line);
            current_value.push('\n');
        }
    }
    if !current_key.is_empty() {
        entries.push(MemoryEntry {
            key: current_key,
            value: current_value.trim().to_string(),
            source: "loaded".into(),
            timestamp: String::new(),
        });
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_extract_memory() {
        let msgs: Vec<Message> = (0..10)
            .map(|_| {
                Message::assistant(vec![ContentBlock::Text {
                    text: "response".into(),
                }])
            })
            .collect();
        assert!(should_extract_memory(&msgs));

        let msgs: Vec<Message> = (0..5)
            .map(|_| {
                Message::assistant(vec![ContentBlock::Text {
                    text: "response".into(),
                }])
            })
            .collect();
        assert!(!should_extract_memory(&msgs));
    }

    #[test]
    fn test_extract_facts() {
        let msgs = vec![Message::assistant(vec![ContentBlock::Text {
            text: "I edited src/main.rs and tests/test.py".into(),
        }])];
        let facts = extract_facts_from_messages(&msgs);
        assert!(facts.iter().any(|f| f.contains("main.rs")));
        assert!(facts.iter().any(|f| f.contains("test.py")));
    }
}
