//! File system operations for reading, writing, and editing files.

use std::path::{Path, PathBuf};

/// Check if a path exists.
pub async fn path_exists(path: &Path) -> bool {
    tokio::fs::metadata(path).await.is_ok()
}

/// Read file safely, returning None on error.
pub fn read_file_safe(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Get file modification time as unix timestamp.
pub fn get_file_mtime(path: &Path) -> Option<u64> {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

/// Get display path (relative to cwd if possible).
pub fn get_display_path(path: &str, cwd: &Path) -> String {
    if let Ok(rel) = Path::new(path).strip_prefix(cwd) {
        rel.to_string_lossy().to_string()
    } else {
        path.to_string()
    }
}

/// Detect line endings in content.
pub fn detect_line_endings(content: &str) -> &'static str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Convert leading tabs to spaces.
pub fn convert_tabs_to_spaces(content: &str, tab_size: usize) -> String {
    content
        .lines()
        .map(|line| {
            let leading_tabs = line.len() - line.trim_start_matches('\t').len();
            format!(
                "{}{}",
                " ".repeat(leading_tabs * tab_size),
                &line[leading_tabs..]
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Resolve path with symlink following for permission checks.
pub fn get_paths_for_permission_check(path: &str) -> Vec<String> {
    let mut paths = vec![path.to_string()];
    // Try to resolve symlinks
    if let Ok(resolved) = std::fs::canonicalize(path) {
        let resolved_str = resolved.to_string_lossy().to_string();
        if resolved_str != path {
            paths.push(resolved_str);
        }
    }
    paths
}

/// Safe path resolution (prevents traversal attacks).
pub fn safe_resolve_path(base: &Path, relative: &str) -> Option<PathBuf> {
    let resolved = base.join(relative);
    let canonical = resolved.canonicalize().ok()?;
    // Ensure resolved path is within base
    if canonical.starts_with(base) {
        Some(canonical)
    } else {
        None
    }
}
