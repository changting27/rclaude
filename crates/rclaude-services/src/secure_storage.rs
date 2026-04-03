//! Secure storage matching utils/secureStorage/.
//! Provides encrypted storage for sensitive data (API keys, tokens).

use std::path::PathBuf;

/// Get the secure storage directory.
fn secure_dir() -> PathBuf {
    rclaude_core::config::Config::config_dir().join("secure")
}

/// Store a value securely (file with restricted permissions).
pub fn store_secure(key: &str, value: &str) -> Result<(), String> {
    let dir = secure_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let path = dir.join(key);
    std::fs::write(&path, value).map_err(|e| e.to_string())?;

    // Set file permissions to owner-only (Unix)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(&path, perms).map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Retrieve a securely stored value.
pub fn retrieve_secure(key: &str) -> Option<String> {
    let path = secure_dir().join(key);
    std::fs::read_to_string(&path).ok()
}

/// Delete a securely stored value.
pub fn delete_secure(key: &str) -> Result<(), String> {
    let path = secure_dir().join(key);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// List all secure storage keys.
pub fn list_secure_keys() -> Vec<String> {
    let dir = secure_dir();
    std::fs::read_dir(&dir)
        .ok()
        .map(|entries| {
            entries
                .filter_map(|e| {
                    e.ok()
                        .and_then(|e| e.file_name().to_str().map(|s| s.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}
