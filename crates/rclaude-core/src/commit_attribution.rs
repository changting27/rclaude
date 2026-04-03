//! Commit attribution for tracking AI-assisted changes.
//! Tracks which files were modified by Claude for git commit attribution.

use std::collections::HashSet;
use std::path::PathBuf;

/// Files modified during a session for commit attribution.
#[derive(Debug, Default)]
pub struct CommitAttribution {
    /// Files written by Claude.
    pub files_written: HashSet<PathBuf>,
    /// Files edited by Claude.
    pub files_edited: HashSet<PathBuf>,
    /// Lines added.
    pub lines_added: u64,
    /// Lines removed.
    pub lines_removed: u64,
}

impl CommitAttribution {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a file write.
    pub fn record_write(&mut self, path: PathBuf) {
        self.files_written.insert(path);
    }

    /// Record a file edit.
    pub fn record_edit(&mut self, path: PathBuf, added: u64, removed: u64) {
        self.files_edited.insert(path);
        self.lines_added += added;
        self.lines_removed += removed;
    }

    /// Get all modified files.
    pub fn all_modified_files(&self) -> Vec<&PathBuf> {
        self.files_written.union(&self.files_edited).collect()
    }

    /// Generate a commit trailer for attribution.
    pub fn commit_trailer(&self) -> String {
        let total = self.files_written.len() + self.files_edited.len();
        if total == 0 {
            return String::new();
        }
        format!(
            "Co-authored-by: Claude <claude@anthropic.com>\n\
             Claude-Modified-Files: {total}\n\
             Claude-Lines-Changed: +{} -{}",
            self.lines_added, self.lines_removed
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attribution() {
        let mut attr = CommitAttribution::new();
        attr.record_write(PathBuf::from("new_file.rs"));
        attr.record_edit(PathBuf::from("existing.rs"), 10, 3);
        assert_eq!(attr.all_modified_files().len(), 2);
        let trailer = attr.commit_trailer();
        assert!(trailer.contains("Co-authored-by: Claude"));
        assert!(trailer.contains("+10 -3"));
    }

    #[test]
    fn test_empty_attribution() {
        let attr = CommitAttribution::new();
        assert!(attr.commit_trailer().is_empty());
    }
}
