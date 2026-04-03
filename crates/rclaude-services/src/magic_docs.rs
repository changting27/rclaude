//! MagicDocs matching services/MagicDocs/.
//! Auto-updates documentation files based on code changes.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Magic doc header pattern.
const MAGIC_DOC_HEADER: &str = "<!-- claude:auto-update -->";

/// Track which files are magic docs.
static TRACKED_DOCS: std::sync::Mutex<Option<HashSet<PathBuf>>> = std::sync::Mutex::new(None);

/// Check if a file has the magic doc header.
pub fn detect_magic_doc_header(content: &str) -> bool {
    content
        .lines()
        .take(5)
        .any(|l| l.trim() == MAGIC_DOC_HEADER)
}

/// Register a file as a magic doc.
pub fn register_magic_doc(path: &Path) {
    let mut tracked = TRACKED_DOCS.lock().unwrap();
    tracked
        .get_or_insert_with(HashSet::new)
        .insert(path.to_path_buf());
}

/// Get all tracked magic docs.
pub fn get_tracked_docs() -> Vec<PathBuf> {
    TRACKED_DOCS
        .lock()
        .ok()
        .and_then(|t| t.as_ref().map(|s| s.iter().cloned().collect()))
        .unwrap_or_default()
}

/// Clear tracked docs.
pub fn clear_tracked_docs() {
    if let Ok(mut tracked) = TRACKED_DOCS.lock() {
        *tracked = None;
    }
}

/// Scan a directory for magic docs.
pub async fn scan_for_magic_docs(dir: &Path) -> Vec<PathBuf> {
    let mut docs = Vec::new();
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return docs,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if let Ok(content) = tokio::fs::read_to_string(&path).await {
            if detect_magic_doc_header(&content) {
                docs.push(path);
            }
        }
    }
    docs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_header() {
        assert!(detect_magic_doc_header(
            "<!-- claude:auto-update -->\n# API Docs"
        ));
        assert!(!detect_magic_doc_header("# Regular Doc\nNo magic here"));
    }

    #[test]
    fn test_register_and_get() {
        clear_tracked_docs();
        register_magic_doc(Path::new("/project/API.md"));
        let docs = get_tracked_docs();
        assert_eq!(docs.len(), 1);
        clear_tracked_docs();
    }
}
