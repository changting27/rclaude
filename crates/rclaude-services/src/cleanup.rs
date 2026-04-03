//! Cleanup service for temporary files and stale sessions.
//! Removes old session files, logs, and caches.

use std::path::Path;
use std::time::{Duration, SystemTime};

const MAX_AGE_DAYS: u64 = 30;
const MAX_SESSION_FILES: usize = 100;

/// Cleanup result.
#[derive(Debug, Default)]
pub struct CleanupResult {
    pub files_removed: usize,
    pub bytes_freed: u64,
    pub errors: Vec<String>,
}

/// Run all cleanup tasks.
pub async fn run_cleanup(_cwd: &Path) -> CleanupResult {
    let mut total = CleanupResult::default();

    // Clean old session files
    let sessions_dir = rclaude_core::config::Config::projects_dir();
    if sessions_dir.exists() {
        let r = cleanup_old_files(&sessions_dir, MAX_AGE_DAYS, MAX_SESSION_FILES).await;
        total.files_removed += r.files_removed;
        total.bytes_freed += r.bytes_freed;
        total.errors.extend(r.errors);
    }

    // Clean old debug logs
    let log_dir = rclaude_core::config::Config::config_dir().join("logs");
    if log_dir.exists() {
        let r = cleanup_old_files(&log_dir, 7, 50).await;
        total.files_removed += r.files_removed;
        total.bytes_freed += r.bytes_freed;
        total.errors.extend(r.errors);
    }

    total
}

/// Remove files older than max_age_days, keeping at most max_files.
async fn cleanup_old_files(dir: &Path, max_age_days: u64, max_files: usize) -> CleanupResult {
    let mut result = CleanupResult::default();
    let max_age = Duration::from_secs(max_age_days * 86400);
    let now = SystemTime::now();

    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(e) => e,
        Err(_) => return result,
    };

    let mut files: Vec<(std::path::PathBuf, SystemTime, u64)> = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if let Ok(meta) = entry.metadata().await {
            let modified = meta.modified().unwrap_or(now);
            files.push((path, modified, meta.len()));
        }
    }

    // Sort by modified time (oldest first)
    files.sort_by_key(|(_, t, _)| *t);

    // Remove old files and excess files
    let excess = files.len().saturating_sub(max_files);
    for (i, (path, modified, size)) in files.iter().enumerate() {
        let age = now.duration_since(*modified).unwrap_or_default();
        if (age > max_age || i < excess) && tokio::fs::remove_file(&path).await.is_ok() {
            result.files_removed += 1;
            result.bytes_freed += size;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_result_default() {
        let r = CleanupResult::default();
        assert_eq!(r.files_removed, 0);
        assert_eq!(r.bytes_freed, 0);
    }
}
