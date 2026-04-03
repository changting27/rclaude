//! Filesystem path permission validation.
//!
//! Validates file paths against working directory constraints, dangerous file
//! lists, and permission rules for read/write operations.

use std::path::Path;

/// Dangerous files that should be protected from auto-editing.
pub const DANGEROUS_FILES: &[&str] = &[
    ".gitconfig",
    ".gitmodules",
    ".bashrc",
    ".bash_profile",
    ".zshrc",
    ".zprofile",
    ".profile",
    ".ripgreprc",
    ".mcp.json",
    ".claude.json",
];

/// Dangerous directories that should be protected from auto-editing.
pub const DANGEROUS_DIRECTORIES: &[&str] = &[".git", ".vscode", ".idea", ".claude"];

/// Claude config file patterns.
const CLAUDE_CONFIG_PATTERNS: &[&str] = &[
    "settings.json",
    "settings.local.json",
    "CLAUDE.md",
    "CLAUDE.local.md",
];

/// Check if a path is a dangerous file to auto-edit.
pub fn is_dangerous_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    let filename = Path::new(&lower)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    DANGEROUS_FILES.iter().any(|f| filename == f.to_lowercase())
}

/// Check if a path is inside a dangerous directory.
pub fn is_in_dangerous_directory(path: &str) -> bool {
    let lower = path.to_lowercase();
    DANGEROUS_DIRECTORIES.iter().any(|d| {
        let pattern = format!("/{}/", d.to_lowercase());
        lower.contains(&pattern) || lower.ends_with(&format!("/{}", d.to_lowercase()))
    })
}

/// Check if a path is a Claude config file.
pub fn is_claude_config_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    // Check if it's in .claude/ directory
    if lower.contains("/.claude/") || lower.ends_with("/.claude") {
        return true;
    }
    let filename = Path::new(&lower)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    CLAUDE_CONFIG_PATTERNS
        .iter()
        .any(|p| filename == p.to_lowercase())
        && lower.contains(".claude")
}

/// Check if a path is within the allowed working directory.
pub fn path_in_working_path(path: &str, working_path: &str) -> bool {
    let abs_path = expand_path(path);
    let abs_working = expand_path(working_path);

    // Normalize macOS symlinks
    let norm_path = normalize_macos_path(&abs_path);
    let norm_working = normalize_macos_path(&abs_working);

    // Case-insensitive comparison
    let lower_path = norm_path.to_lowercase();
    let lower_working = norm_working.to_lowercase();

    // Path must be within or equal to working path
    if lower_path == lower_working {
        return true;
    }
    lower_path.starts_with(&format!("{}/", lower_working))
}

/// Check if a path is within any of the allowed working directories.
pub fn path_in_allowed_dirs(path: &str, working_dirs: &[&str]) -> bool {
    working_dirs.iter().any(|wd| path_in_working_path(path, wd))
}

/// Safety check result for auto-edit.
#[derive(Debug)]
pub enum PathSafety {
    Safe,
    Unsafe { message: String },
}

/// Check if a path is safe for auto-editing (matching checkPathSafetyForAutoEdit).
pub fn check_path_safety_for_auto_edit(path: &str, _cwd: &str) -> PathSafety {
    // Check for Claude config files
    if is_claude_config_path(path) {
        return PathSafety::Unsafe {
            message: format!("Write to Claude config file: {path}"),
        };
    }

    // Check for dangerous files
    if is_dangerous_file(path) {
        return PathSafety::Unsafe {
            message: format!("Edit of sensitive file: {path}"),
        };
    }

    // Check for dangerous directories
    if is_in_dangerous_directory(path) {
        return PathSafety::Unsafe {
            message: format!("Edit in sensitive directory: {path}"),
        };
    }

    PathSafety::Safe
}

/// Permission check result for file operations.
#[derive(Debug)]
pub enum FilePermission {
    Allow,
    Deny(String),
    Ask(String),
}

/// Check read permission for a file path.
pub fn check_read_permission(path: &str, cwd: &str) -> FilePermission {
    let abs = expand_path(path);

    // UNC paths
    if abs.starts_with("\\\\") || abs.starts_with("//") {
        return FilePermission::Ask(format!("UNC path detected: {path}"));
    }

    // Paths within working directory are always allowed for read
    if path_in_working_path(&abs, cwd) {
        return FilePermission::Allow;
    }

    // Paths in home directory are allowed for read
    if let Some(home) = dirs::home_dir() {
        if path_in_working_path(&abs, &home.to_string_lossy()) {
            return FilePermission::Allow;
        }
    }

    // /tmp is allowed for read
    if path_in_working_path(&abs, "/tmp") {
        return FilePermission::Allow;
    }

    // Other paths need approval
    FilePermission::Ask(format!("Read from outside project: {path}"))
}

/// Check write permission for a file path.
pub fn check_write_permission(path: &str, cwd: &str) -> FilePermission {
    let abs = expand_path(path);

    // UNC paths
    if abs.starts_with("\\\\") || abs.starts_with("//") {
        return FilePermission::Deny("UNC path write blocked".into());
    }

    // Safety check for auto-edit
    match check_path_safety_for_auto_edit(&abs, cwd) {
        PathSafety::Unsafe { message } => {
            return FilePermission::Ask(message);
        }
        PathSafety::Safe => {}
    }

    // Must be within working directory, home, or /tmp
    if path_in_working_path(&abs, cwd) {
        return FilePermission::Allow;
    }
    if let Some(home) = dirs::home_dir() {
        if path_in_working_path(&abs, &home.to_string_lossy()) {
            return FilePermission::Allow;
        }
    }
    if path_in_working_path(&abs, "/tmp") {
        return FilePermission::Allow;
    }

    FilePermission::Ask(format!("Write to outside project: {path}"))
}

// ── Helpers ──

fn expand_path(path: &str) -> String {
    let expanded = if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            path.replacen('~', &home.to_string_lossy(), 1)
        } else {
            path.to_string()
        }
    } else if Path::new(path).is_relative() {
        std::env::current_dir()
            .map(|cwd| cwd.join(path).to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string())
    } else {
        path.to_string()
    };
    // Normalize .. and .
    normalize_path(&expanded)
}

fn normalize_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            c => parts.push(c),
        }
    }
    if path.starts_with('/') {
        format!("/{}", parts.join("/"))
    } else {
        parts.join("/")
    }
}

fn normalize_macos_path(path: &str) -> String {
    path.replace("/private/var/", "/var/")
        .replace("/private/tmp/", "/tmp/")
        .replace("/private/tmp", "/tmp")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dangerous_files() {
        assert!(is_dangerous_file("/home/user/.bashrc"));
        assert!(is_dangerous_file("/project/.gitconfig"));
        assert!(!is_dangerous_file("/project/src/main.rs"));
    }

    #[test]
    fn test_dangerous_directories() {
        assert!(is_in_dangerous_directory("/project/.git/config"));
        assert!(is_in_dangerous_directory("/project/.claude/settings.json"));
        assert!(!is_in_dangerous_directory("/project/src/main.rs"));
    }

    #[test]
    fn test_claude_config_path() {
        assert!(is_claude_config_path("/project/.claude/settings.json"));
        assert!(is_claude_config_path(
            "/project/.claude/settings.local.json"
        ));
        assert!(!is_claude_config_path("/project/settings.json"));
    }

    #[test]
    fn test_path_in_working_path() {
        assert!(path_in_working_path(
            "/home/user/project/src/main.rs",
            "/home/user/project"
        ));
        assert!(path_in_working_path(
            "/home/user/project",
            "/home/user/project"
        ));
        assert!(!path_in_working_path("/etc/passwd", "/home/user/project"));
    }

    #[test]
    fn test_path_safety() {
        assert!(matches!(
            check_path_safety_for_auto_edit("/project/src/main.rs", "/project"),
            PathSafety::Safe
        ));
        assert!(matches!(
            check_path_safety_for_auto_edit("/project/.git/config", "/project"),
            PathSafety::Unsafe { .. }
        ));
        assert!(matches!(
            check_path_safety_for_auto_edit("/home/user/.bashrc", "/project"),
            PathSafety::Unsafe { .. }
        ));
    }

    #[test]
    fn test_read_permission() {
        assert!(matches!(
            check_read_permission("/project/src/main.rs", "/project"),
            FilePermission::Allow
        ));
        assert!(matches!(
            check_read_permission("/tmp/test.txt", "/project"),
            FilePermission::Allow
        ));
    }

    #[test]
    fn test_write_permission() {
        assert!(matches!(
            check_write_permission("/project/src/main.rs", "/project"),
            FilePermission::Allow
        ));
        assert!(matches!(
            check_write_permission("/etc/passwd", "/project"),
            FilePermission::Ask(_)
        ));
        assert!(matches!(
            check_write_permission("/project/.git/config", "/project"),
            FilePermission::Ask(_)
        ));
    }

    #[test]
    fn test_macos_normalization() {
        assert_eq!(normalize_macos_path("/private/tmp/test"), "/tmp/test");
        assert_eq!(normalize_macos_path("/private/var/folders"), "/var/folders");
    }
}
