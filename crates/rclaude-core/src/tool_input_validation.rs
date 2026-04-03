//! Tool input validation utilities.

use std::path::{Path, PathBuf};

/// Validate that a required string field exists and is non-empty.
pub fn require_string(input: &serde_json::Value, field: &str) -> Result<String, String> {
    input
        .get(field)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Missing required field: {field}"))
}

/// Validate and resolve a file path from tool input.
/// Resolves relative paths against cwd, normalizes, and checks for traversal.
pub fn validate_file_path(
    input: &serde_json::Value,
    field: &str,
    cwd: &Path,
) -> Result<PathBuf, String> {
    let raw = require_string(input, field)?;
    let resolved = if Path::new(&raw).is_absolute() {
        PathBuf::from(&raw)
    } else {
        cwd.join(&raw)
    };

    // Normalize (resolve .. and .)
    let normalized = normalize_path(&resolved);

    // Block /dev, /proc, /sys
    let s = normalized.to_string_lossy();
    if s.starts_with("/dev/") || s.starts_with("/proc/") || s.starts_with("/sys/") {
        return Err(format!("Access to system path denied: {s}"));
    }

    Ok(normalized)
}

/// Normalize a path (resolve . and ..).
fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            c => components.push(c),
        }
    }
    components.iter().collect()
}

/// Validate an optional u64 field with bounds.
pub fn validate_u64(input: &serde_json::Value, field: &str, default: u64, max: u64) -> u64 {
    input
        .get(field)
        .and_then(|v| v.as_u64())
        .unwrap_or(default)
        .min(max)
}

/// Validate an optional bool field.
pub fn validate_bool(input: &serde_json::Value, field: &str, default: bool) -> bool {
    input
        .get(field)
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_require_string() {
        let input = json!({"name": "hello"});
        assert_eq!(require_string(&input, "name").unwrap(), "hello");
        assert!(require_string(&input, "missing").is_err());
    }

    #[test]
    fn test_require_string_empty() {
        let input = json!({"name": ""});
        assert!(require_string(&input, "name").is_err());
    }

    #[test]
    fn test_validate_file_path_relative() {
        let input = json!({"path": "src/main.rs"});
        let result = validate_file_path(&input, "path", Path::new("/home/user/project")).unwrap();
        assert_eq!(result, PathBuf::from("/home/user/project/src/main.rs"));
    }

    #[test]
    fn test_validate_file_path_absolute() {
        let input = json!({"path": "/tmp/test.txt"});
        let result = validate_file_path(&input, "path", Path::new("/home/user")).unwrap();
        assert_eq!(result, PathBuf::from("/tmp/test.txt"));
    }

    #[test]
    fn test_validate_file_path_blocked() {
        let input = json!({"path": "/dev/zero"});
        assert!(validate_file_path(&input, "path", Path::new("/tmp")).is_err());
    }

    #[test]
    fn test_validate_file_path_traversal() {
        let input = json!({"path": "../../../etc/passwd"});
        let result = validate_file_path(&input, "path", Path::new("/home/user/project")).unwrap();
        assert_eq!(result, PathBuf::from("/etc/passwd"));
    }

    #[test]
    fn test_validate_u64() {
        let input = json!({"timeout": 5000});
        assert_eq!(validate_u64(&input, "timeout", 1000, 10000), 5000);
        assert_eq!(validate_u64(&input, "missing", 1000, 10000), 1000);
        assert_eq!(validate_u64(&json!({"x": 99999}), "x", 0, 100), 100);
    }
}
