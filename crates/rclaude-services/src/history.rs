//! Input history: persistent command history across sessions.

use std::path::PathBuf;

const MAX_HISTORY_SIZE: usize = 1000;

/// Persistent input history.
pub struct InputHistory {
    entries: Vec<String>,
    position: Option<usize>,
    file_path: PathBuf,
}

impl InputHistory {
    /// Load history from disk (JSONL format, compatible with claude's history.jsonl).
    pub fn load() -> Self {
        let file_path = rclaude_core::config::Config::config_dir().join("history.jsonl");
        let entries = std::fs::read_to_string(&file_path)
            .map(|content| {
                content
                    .lines()
                    .filter(|l| !l.is_empty())
                    .filter_map(|l| {
                        // JSONL: each line is {"prompt":"...","timestamp":...}
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(l) {
                            val.get("prompt")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string())
                        } else {
                            // Fallback: plain text line
                            Some(l.to_string())
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        Self {
            entries,
            position: None,
            file_path,
        }
    }

    /// Add an entry to history (dedup consecutive).
    pub fn push(&mut self, entry: String) {
        if entry.is_empty() {
            return;
        }
        if self.entries.last().is_some_and(|last| last == &entry) {
            return;
        }
        self.entries.push(entry);
        if self.entries.len() > MAX_HISTORY_SIZE {
            self.entries.drain(..self.entries.len() - MAX_HISTORY_SIZE);
        }
        self.position = None;
    }

    /// Navigate to previous entry.
    pub fn prev(&mut self) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        let pos = match self.position {
            Some(p) if p > 0 => p - 1,
            Some(_) => return None,
            None => self.entries.len() - 1,
        };
        self.position = Some(pos);
        Some(&self.entries[pos])
    }

    /// Navigate to next entry.
    pub fn next_entry(&mut self) -> Option<&str> {
        let pos = self.position?;
        if pos + 1 < self.entries.len() {
            self.position = Some(pos + 1);
            Some(&self.entries[pos + 1])
        } else {
            self.position = None;
            None
        }
    }

    /// Search history for entries containing query.
    pub fn search(&self, query: &str) -> Vec<&str> {
        let q = query.to_lowercase();
        self.entries
            .iter()
            .rev()
            .filter(|e| e.to_lowercase().contains(&q))
            .map(|e| e.as_str())
            .take(20)
            .collect()
    }

    /// Save history to disk in JSONL format (compatible with claude).
    pub fn save(&self) {
        if let Some(parent) = self.file_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut content = String::new();
        for entry in &self.entries {
            let line = serde_json::json!({
                "prompt": entry,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            });
            content.push_str(&serde_json::to_string(&line).unwrap_or_default());
            content.push('\n');
        }
        let _ = std::fs::write(&self.file_path, content);
    }

    /// Get total entry count.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Drop for InputHistory {
    fn drop(&mut self) {
        self.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_and_navigate() {
        let mut h = InputHistory {
            entries: vec![],
            position: None,
            file_path: PathBuf::from("/tmp/test_history"),
        };
        h.push("first".into());
        h.push("second".into());
        h.push("third".into());

        assert_eq!(h.prev(), Some("third"));
        assert_eq!(h.prev(), Some("second"));
        assert_eq!(h.prev(), Some("first"));
        assert_eq!(h.prev(), None);

        assert_eq!(h.next_entry(), Some("second"));
        assert_eq!(h.next_entry(), Some("third"));
        assert_eq!(h.next_entry(), None);
    }

    #[test]
    fn test_dedup_consecutive() {
        let mut h = InputHistory {
            entries: vec![],
            position: None,
            file_path: PathBuf::from("/tmp/test_history"),
        };
        h.push("same".into());
        h.push("same".into());
        h.push("same".into());
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn test_search() {
        let mut h = InputHistory {
            entries: vec![],
            position: None,
            file_path: PathBuf::from("/tmp/test_history"),
        };
        h.push("git status".into());
        h.push("cargo build".into());
        h.push("git push".into());
        let results = h.search("git");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], "git push"); // most recent first
    }

    #[test]
    fn test_max_size() {
        let mut h = InputHistory {
            entries: vec![],
            position: None,
            file_path: PathBuf::from("/tmp/test_history"),
        };
        for i in 0..1500 {
            h.push(format!("entry {i}"));
        }
        assert_eq!(h.len(), MAX_HISTORY_SIZE);
    }
}
