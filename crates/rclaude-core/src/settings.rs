//! Multi-source settings system with validation and change detection.
//! Multi-source settings with validation and change detection.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Setting sources in priority order (later overrides earlier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SettingSource {
    UserSettings,    // ~/.claude/settings.json
    ProjectSettings, // .claude/settings.json
    LocalSettings,   // .claude/settings.local.json
    PolicySettings,  // /etc/claude-code/settings.json
}

impl SettingSource {
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::UserSettings => "User",
            Self::ProjectSettings => "Project",
            Self::LocalSettings => "Local",
            Self::PolicySettings => "Managed",
        }
    }

    pub fn file_path(&self, cwd: &Path) -> PathBuf {
        match self {
            Self::UserSettings => dirs::home_dir()
                .unwrap_or_default()
                .join(".claude/settings.json"),
            Self::ProjectSettings => cwd.join(".claude/settings.json"),
            Self::LocalSettings => cwd.join(".claude/settings.local.json"),
            Self::PolicySettings => PathBuf::from("/etc/claude-code/settings.json"),
        }
    }

    /// All sources in priority order.
    pub fn all() -> &'static [SettingSource] {
        &[
            Self::UserSettings,
            Self::ProjectSettings,
            Self::LocalSettings,
            Self::PolicySettings,
        ]
    }

    /// Editable sources (user can modify).
    pub fn editable() -> &'static [SettingSource] {
        &[
            Self::UserSettings,
            Self::ProjectSettings,
            Self::LocalSettings,
        ]
    }
}

/// Merged settings from all sources.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default, rename = "availableModels")]
    pub available_models: Vec<String>,
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default, rename = "editorMode")]
    pub editor_mode: Option<String>,
    #[serde(default, rename = "defaultShell")]
    pub default_shell: Option<String>,
    // Tool permissions
    #[serde(default, rename = "allowedTools")]
    pub allowed_tools: Vec<String>,
    #[serde(default, rename = "deniedTools")]
    pub denied_tools: Vec<String>,
    // Permission rules
    #[serde(default)]
    pub permissions: Vec<serde_json::Value>,
    // Hooks configuration
    #[serde(default)]
    pub hooks: serde_json::Value,
    // Environment variables
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    // API key helper script
    #[serde(default, rename = "apiKeyHelper")]
    pub api_key_helper: Option<String>,
    #[serde(default, rename = "customApiKey")]
    pub custom_api_key: Option<String>,
    // MCP server configuration
    #[serde(default, rename = "allowedMcpServers")]
    pub allowed_mcp_servers: Vec<serde_json::Value>,
    #[serde(default, rename = "deniedMcpServers")]
    pub denied_mcp_servers: Vec<serde_json::Value>,
    #[serde(default, rename = "enableAllProjectMcpServers")]
    pub enable_all_project_mcp_servers: Option<bool>,
    // Worktree settings
    #[serde(default)]
    pub worktree: Option<serde_json::Value>,
    // Attribution settings
    #[serde(default)]
    pub attribution: Option<serde_json::Value>,
    // Cleanup
    #[serde(default, rename = "cleanupPeriodDays")]
    pub cleanup_period_days: Option<u32>,
    // Disable all hooks (managed policy)
    #[serde(default, rename = "disableAllHooks")]
    pub disable_all_hooks: Option<bool>,
    #[serde(default, rename = "allowManagedHooksOnly")]
    pub allow_managed_hooks_only: Option<bool>,
    #[serde(default, rename = "allowManagedPermissionRulesOnly")]
    pub allow_managed_permission_rules_only: Option<bool>,
    // Verbose
    #[serde(default)]
    pub verbose: Option<bool>,
    // Catch-all for unknown fields
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Load settings from a single source.
pub fn load_settings_from_source(source: SettingSource, cwd: &Path) -> Option<Settings> {
    let path = source.file_path(cwd);
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Load and merge settings from all sources.
pub fn load_merged_settings(cwd: &Path) -> Settings {
    let mut merged = Settings::default();
    for source in SettingSource::all() {
        if let Some(s) = load_settings_from_source(*source, cwd) {
            merge_settings(&mut merged, &s);
        }
    }
    merged
}

/// Merge source settings into target (non-None values override).
fn merge_settings(target: &mut Settings, source: &Settings) {
    if source.model.is_some() {
        target.model = source.model.clone();
    }
    if source.theme.is_some() {
        target.theme = source.theme.clone();
    }
    if source.editor_mode.is_some() {
        target.editor_mode = source.editor_mode.clone();
    }
    if !source.allowed_tools.is_empty() {
        target.allowed_tools.extend(source.allowed_tools.clone());
    }
    if !source.denied_tools.is_empty() {
        target.denied_tools.extend(source.denied_tools.clone());
    }
    if source.custom_api_key.is_some() {
        target.custom_api_key = source.custom_api_key.clone();
    }
    if source.verbose.is_some() {
        target.verbose = source.verbose;
    }
    for (k, v) in &source.extra {
        target.extra.insert(k.clone(), v.clone());
    }
}

/// Save a setting to a specific source.
pub fn save_setting(
    source: SettingSource,
    cwd: &Path,
    key: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    let path = source.file_path(cwd);
    let mut settings: serde_json::Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({}));

    settings[key] = value;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setting_source_paths() {
        let cwd = Path::new("/project");
        assert!(SettingSource::UserSettings
            .file_path(cwd)
            .to_string_lossy()
            .contains(".claude"));
        assert!(SettingSource::ProjectSettings
            .file_path(cwd)
            .to_string_lossy()
            .contains("/project/.claude"));
    }

    #[test]
    fn test_merge_settings() {
        let mut base = Settings {
            model: Some("sonnet".into()),
            ..Default::default()
        };
        let overlay = Settings {
            model: Some("opus".into()),
            theme: Some("dark".into()),
            ..Default::default()
        };
        merge_settings(&mut base, &overlay);
        assert_eq!(base.model, Some("opus".into()));
        assert_eq!(base.theme, Some("dark".into()));
    }

    #[test]
    fn test_setting_source_display() {
        assert_eq!(SettingSource::UserSettings.display_name(), "User");
        assert_eq!(SettingSource::PolicySettings.display_name(), "Managed");
    }
}
