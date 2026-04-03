//! Memory extraction matching services/extractMemories/.
//! Automatically extracts key learnings from conversations.

use rclaude_core::message::{ContentBlock, Message, Role};
use std::path::Path;

/// Extract memories from conversation messages.
pub async fn extract_memories(messages: &[Message], _cwd: &Path) -> Vec<ExtractedMemory> {
    let mut memories = Vec::new();

    for msg in messages {
        if msg.role != Role::Assistant {
            continue;
        }
        for block in &msg.content {
            if let ContentBlock::Text { text } = block {
                // Extract patterns, conventions, and preferences
                memories.extend(extract_from_text(text));
            }
        }
    }

    memories.dedup_by(|a, b| a.content == b.content);
    memories.truncate(20); // Max 20 memories per extraction
    memories
}

/// A single extracted memory.
#[derive(Debug, Clone)]
pub struct ExtractedMemory {
    pub content: String,
    pub category: MemoryCategory,
    pub confidence: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryCategory {
    Convention, // Code conventions
    Preference, // User preferences
    Pattern,    // Code patterns
    Tool,       // Tool usage patterns
}

fn extract_from_text(text: &str) -> Vec<ExtractedMemory> {
    let mut memories = Vec::new();

    // Extract file path patterns
    for line in text.lines() {
        let trimmed = line.trim();

        // Convention markers
        if (trimmed.contains("convention")
            || trimmed.contains("always use")
            || trimmed.contains("prefer"))
            && trimmed.len() > 10
            && trimmed.len() < 200
        {
            memories.push(ExtractedMemory {
                content: trimmed.to_string(),
                category: MemoryCategory::Convention,
                confidence: 0.7,
            });
        }

        // Error fix patterns
        if (trimmed.contains("fixed by")
            || trimmed.contains("the fix was")
            || trimmed.contains("resolved by"))
            && trimmed.len() > 10
            && trimmed.len() < 200
        {
            memories.push(ExtractedMemory {
                content: trimmed.to_string(),
                category: MemoryCategory::Pattern,
                confidence: 0.6,
            });
        }
    }

    memories
}

/// Save extracted memories to the auto-memory file.
pub async fn save_extracted_memories(
    cwd: &Path,
    memories: &[ExtractedMemory],
) -> Result<(), String> {
    if memories.is_empty() {
        return Ok(());
    }

    let mem_dir = cwd.join(".claude");
    tokio::fs::create_dir_all(&mem_dir)
        .await
        .map_err(|e| e.to_string())?;

    let path = mem_dir.join("auto-memory.md");
    let mut content = tokio::fs::read_to_string(&path).await.unwrap_or_default();

    for mem in memories {
        let category = match mem.category {
            MemoryCategory::Convention => "convention",
            MemoryCategory::Preference => "preference",
            MemoryCategory::Pattern => "pattern",
            MemoryCategory::Tool => "tool",
        };
        content.push_str(&format!("\n- [{category}] {}\n", mem.content));
    }

    tokio::fs::write(&path, content)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_from_text() {
        let text = "We prefer to always use conventional commits for this project.\nThe issue was resolved by adding a null check to the handler.";
        let memories = extract_from_text(text);
        assert!(!memories.is_empty());
    }
}
