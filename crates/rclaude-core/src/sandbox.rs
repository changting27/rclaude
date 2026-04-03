//! Sandbox support matching utils/sandbox/.
//! Provides isolated execution environments.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Sandbox configuration.
#[derive(Debug, Clone, Default)]
pub struct SandboxConfig {
    /// Allowed read paths.
    pub read_paths: HashSet<PathBuf>,
    /// Allowed write paths.
    pub write_paths: HashSet<PathBuf>,
    /// Allowed network domains.
    pub allowed_domains: HashSet<String>,
    /// Whether sandbox is enabled.
    pub enabled: bool,
}

impl SandboxConfig {
    /// Create a sandbox config for a project directory.
    pub fn for_project(cwd: &Path) -> Self {
        let mut config = Self::default();
        config.read_paths.insert(cwd.to_path_buf());
        config.write_paths.insert(cwd.to_path_buf());
        config.read_paths.insert(PathBuf::from("/tmp"));
        config.write_paths.insert(PathBuf::from("/tmp"));
        if let Some(home) = dirs::home_dir() {
            config.read_paths.insert(home.join(".claude"));
        }
        config
    }

    /// Check if a path is allowed for reading.
    pub fn can_read(&self, path: &Path) -> bool {
        if !self.enabled {
            return true;
        }
        self.read_paths
            .iter()
            .any(|allowed| path.starts_with(allowed))
    }

    /// Check if a path is allowed for writing.
    pub fn can_write(&self, path: &Path) -> bool {
        if !self.enabled {
            return true;
        }
        self.write_paths
            .iter()
            .any(|allowed| path.starts_with(allowed))
    }

    /// Check if a domain is allowed for network access.
    pub fn can_access_domain(&self, domain: &str) -> bool {
        if !self.enabled {
            return true;
        }
        self.allowed_domains.contains(domain) || self.allowed_domains.contains("*")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_allows_all() {
        let config = SandboxConfig::default();
        assert!(config.can_read(Path::new("/etc/passwd")));
        assert!(config.can_write(Path::new("/etc/passwd")));
    }

    #[test]
    fn test_enabled_restricts() {
        let mut config = SandboxConfig::for_project(Path::new("/project"));
        config.enabled = true;
        assert!(config.can_read(Path::new("/project/src/main.rs")));
        assert!(!config.can_read(Path::new("/etc/passwd")));
        assert!(config.can_write(Path::new("/project/output.txt")));
        assert!(!config.can_write(Path::new("/etc/shadow")));
    }
}
