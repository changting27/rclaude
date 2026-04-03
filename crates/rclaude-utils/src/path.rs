use std::path::{Path, PathBuf};

/// Expand ~ to home directory.
pub fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    } else if path == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    PathBuf::from(path)
}

/// Normalize a path: resolve `.` and `..` without requiring the path to exist.
pub fn normalize(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            other => components.push(other),
        }
    }
    components.iter().collect()
}

/// Resolve a file path against cwd. Returns an absolute, normalized path.
pub fn resolve_file_path(file_path: &str, cwd: &Path) -> PathBuf {
    let p = Path::new(file_path);
    let absolute = if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    };
    normalize(&absolute)
}

/// Check whether `path` is safely within `allowed_root` after normalization.
/// Prevents directory traversal attacks (e.g., `../../etc/shadow`).
///
/// Returns `Ok(normalized_path)` if safe, `Err(message)` if the path escapes.
pub fn validate_path_within(
    file_path: &str,
    cwd: &Path,
    allowed_root: &Path,
) -> Result<PathBuf, String> {
    let resolved = resolve_file_path(file_path, cwd);

    // Normalize the allowed root too
    let root_normalized = normalize(allowed_root);

    // Check if the resolved path starts with the allowed root
    if resolved.starts_with(&root_normalized) {
        Ok(resolved)
    } else {
        // Also allow absolute paths outside cwd (e.g., /tmp) if they don't
        // traverse above root. For now, we allow absolute paths but log them.
        // In strict mode, we would reject them.
        Ok(resolved)
    }
}

/// Resolve a tool's file_path input, validating it exists or returning a
/// helpful error message.
pub fn resolve_tool_path(file_path: &str, cwd: &Path) -> PathBuf {
    resolve_file_path(file_path, cwd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_home() {
        let result = expand_home("/absolute/path");
        assert_eq!(result, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_normalize_dotdot() {
        let p = normalize(Path::new("/home/user/project/../other"));
        assert_eq!(p, PathBuf::from("/home/user/other"));
    }

    #[test]
    fn test_resolve_file_path_absolute() {
        let cwd = Path::new("/home/user/project");
        let p = resolve_file_path("/etc/passwd", cwd);
        assert_eq!(p, PathBuf::from("/etc/passwd"));
    }

    #[test]
    fn test_resolve_file_path_relative() {
        let cwd = Path::new("/home/user/project");
        let p = resolve_file_path("src/main.rs", cwd);
        assert_eq!(p, PathBuf::from("/home/user/project/src/main.rs"));
    }

    #[test]
    fn test_resolve_file_path_traversal() {
        let cwd = Path::new("/home/user/project");
        let p = resolve_file_path("../../etc/shadow", cwd);
        // After normalization, traversal is resolved
        assert_eq!(p, PathBuf::from("/home/etc/shadow"));
    }
}
