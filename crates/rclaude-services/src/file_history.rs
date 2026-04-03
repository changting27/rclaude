//! File history tracking service.
//! Records file modifications made by tools for potential rollback.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A snapshot of a file before modification.
#[derive(Debug, Clone)]
pub struct FileSnapshot {
    pub path: PathBuf,
    pub content: Option<String>, // None if file didn't exist
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Tracks file modifications during a session.
#[derive(Debug, Default)]
pub struct FileHistory {
    /// Map from file path to list of snapshots (oldest first).
    snapshots: HashMap<PathBuf, Vec<FileSnapshot>>,
}

impl FileHistory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a file's content before modification.
    pub async fn snapshot_before_write(&mut self, path: &Path) {
        let content = tokio::fs::read_to_string(path).await.ok();
        let snapshot = FileSnapshot {
            path: path.to_path_buf(),
            content,
            timestamp: chrono::Utc::now(),
        };
        self.snapshots
            .entry(path.to_path_buf())
            .or_default()
            .push(snapshot);
    }

    /// Get the most recent snapshot for a file.
    pub fn last_snapshot(&self, path: &Path) -> Option<&FileSnapshot> {
        self.snapshots.get(path)?.last()
    }

    /// Get all modified file paths.
    pub fn modified_files(&self) -> Vec<&Path> {
        self.snapshots.keys().map(|p| p.as_path()).collect()
    }

    /// Get total number of snapshots.
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.values().map(|v| v.len()).sum()
    }
}

/// Convenience: record a snapshot of a file's content (standalone, no FileHistory needed).
/// Useful for tools that want to record before-state without access to the session's FileHistory.
pub async fn record_snapshot(path: &Path, content: &str) {
    // Write to a shadow file in .claude/file-history/ for potential recovery
    let hash = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        path.hash(&mut h);
        h.finish()
    };
    let shadow_dir = rclaude_core::config::Config::config_dir().join("file-history");
    if std::fs::create_dir_all(&shadow_dir).is_ok() {
        let shadow_path = shadow_dir.join(format!("{hash:x}.bak"));
        let _ = tokio::fs::write(&shadow_path, content).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_snapshot_nonexistent_file() {
        let mut history = FileHistory::new();
        history
            .snapshot_before_write(Path::new("/tmp/__nonexistent_test_42__"))
            .await;
        let snap = history
            .last_snapshot(Path::new("/tmp/__nonexistent_test_42__"))
            .unwrap();
        assert!(snap.content.is_none());
    }

    #[tokio::test]
    async fn test_snapshot_existing_file() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "original content").unwrap();

        let mut history = FileHistory::new();
        history.snapshot_before_write(tmp.path()).await;
        let snap = history.last_snapshot(tmp.path()).unwrap();
        assert_eq!(snap.content.as_deref(), Some("original content"));
    }

    #[test]
    fn test_modified_files() {
        let mut history = FileHistory::new();
        history.snapshots.insert(PathBuf::from("/a"), vec![]);
        history.snapshots.insert(PathBuf::from("/b"), vec![]);
        assert_eq!(history.modified_files().len(), 2);
    }
}
