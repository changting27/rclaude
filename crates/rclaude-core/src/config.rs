use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Application configuration loaded from multiple layers.
///
/// Loading order:
/// 1. Defaults
/// 2. Global config (~/.claude/settings.json)
/// 3. Project config (.claude/settings.json in cwd)
/// 4. Environment variables (ANTHROPIC_API_KEY, CLAUDE_MODEL, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// API key for Anthropic.
    #[serde(default, alias = "apiKey")]
    pub api_key: Option<String>,
    /// Default model to use.
    #[serde(default = "default_model")]
    pub model: String,
    /// Maximum tokens for output.
    #[serde(default = "default_max_tokens", alias = "maxTokens")]
    pub max_tokens: u32,
    /// Whether to enable verbose output.
    #[serde(default)]
    pub verbose: bool,
    /// Custom API base URL.
    #[serde(default, alias = "apiBaseUrl")]
    pub api_base_url: Option<String>,
    /// Theme name.
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Allowed tools (permission allowlist).
    #[serde(default, alias = "allowedTools")]
    pub allowed_tools: Vec<String>,
    /// Denied tools (permission denylist).
    #[serde(default, alias = "deniedTools")]
    pub denied_tools: Vec<String>,
    /// Environment variables to set for tool execution.
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    /// Default permission mode.
    #[serde(default, alias = "permissionMode")]
    pub permission_mode: Option<String>,
    /// Output style (default, explanatory, learning).
    #[serde(default, alias = "outputStyle")]
    pub output_style: Option<String>,
    /// Hooks configuration.
    #[serde(default)]
    pub hooks: Option<serde_json::Value>,
    /// Custom system prompt to prepend.
    #[serde(default, alias = "systemPrompt")]
    pub system_prompt: Option<String>,
    /// Max turns for agentic loop.
    #[serde(default, alias = "maxTurns")]
    pub max_turns: Option<u32>,
}

fn default_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}

fn default_max_tokens() -> u32 {
    16384
}

fn default_theme() -> String {
    "dark".to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: None,
            model: default_model(),
            max_tokens: default_max_tokens(),
            verbose: false,
            api_base_url: None,
            theme: default_theme(),
            allowed_tools: Vec::new(),
            denied_tools: Vec::new(),
            env: std::collections::HashMap::new(),
            permission_mode: None,
            output_style: None,
            hooks: None,
            system_prompt: None,
            max_turns: None,
        }
    }
}

impl Config {
    /// Load config with multi-layer merging: defaults → global → project → env.
    pub fn load() -> Self {
        Self::load_for_cwd(&std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    /// Load config for a specific working directory.
    pub fn load_for_cwd(cwd: &Path) -> Self {
        let mut cfg = Self::default();

        // Layer 0: Managed settings (/etc/claude-code/managed-settings.json)
        let managed_path = Path::new("/etc/claude-code/managed-settings.json");
        if let Some(managed) = Self::load_file(managed_path) {
            cfg.merge(managed);
        }

        // Layer 1: Global config (~/.claude/settings.json)
        let global_path = Self::config_dir().join("settings.json");
        if let Some(global) = Self::load_file(&global_path) {
            cfg.merge(global);
        }

        // Layer 2: Project config (.claude/settings.json in cwd)
        let project_path = cwd.join(".claude/settings.json");
        if let Some(project) = Self::load_file(&project_path) {
            cfg.merge(project);
        }

        // Layer 3: Local config (.claude/settings.local.json — gitignored)
        let local_path = cwd.join(".claude/settings.local.json");
        if let Some(local) = Self::load_file(&local_path) {
            cfg.merge(local);
        }

        // Layer 4: Environment variable overrides
        cfg.apply_env_overrides();

        cfg
    }

    /// Load and parse a single config file, returning None on any error.
    fn load_file(path: &Path) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Merge another config into this one (non-default values override).
    fn merge(&mut self, other: Self) {
        if other.api_key.is_some() {
            self.api_key = other.api_key;
        }
        if other.model != default_model() {
            self.model = other.model;
        }
        if other.max_tokens != default_max_tokens() {
            self.max_tokens = other.max_tokens;
        }
        if other.verbose {
            self.verbose = true;
        }
        if other.api_base_url.is_some() {
            self.api_base_url = other.api_base_url;
        }
        if other.theme != default_theme() {
            self.theme = other.theme;
        }
        if !other.allowed_tools.is_empty() {
            self.allowed_tools.extend(other.allowed_tools);
        }
        if !other.denied_tools.is_empty() {
            self.denied_tools.extend(other.denied_tools);
        }
        if !other.env.is_empty() {
            self.env.extend(other.env);
        }
        if other.permission_mode.is_some() {
            self.permission_mode = other.permission_mode;
        }
        if other.output_style.is_some() {
            self.output_style = other.output_style;
        }
        if other.hooks.is_some() {
            self.hooks = other.hooks;
        }
        if other.system_prompt.is_some() {
            self.system_prompt = other.system_prompt;
        }
        if other.max_turns.is_some() {
            self.max_turns = other.max_turns;
        }
    }

    /// Apply environment variable overrides (highest priority).
    fn apply_env_overrides(&mut self) {
        if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            if !key.is_empty() {
                self.api_key = Some(key);
            }
        }
        if let Ok(model) = std::env::var("CLAUDE_MODEL") {
            if !model.is_empty() {
                self.model = model;
            }
        }
        if let Ok(url) = std::env::var("ANTHROPIC_BASE_URL") {
            if !url.is_empty() {
                self.api_base_url = Some(url);
            }
        }
        if let Ok(tokens) = std::env::var("CLAUDE_MAX_TOKENS") {
            if let Ok(n) = tokens.parse() {
                self.max_tokens = n;
            }
        }
    }

    /// Get the config directory path.
    /// Respects CLAUDE_CONFIG_DIR env var, defaults to ~/.claude/
    pub fn config_dir() -> PathBuf {
        if let Ok(dir) = std::env::var("CLAUDE_CONFIG_DIR") {
            return PathBuf::from(dir);
        }
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".claude")
    }

    /// Get the projects directory path (~/.claude/projects/).
    pub fn projects_dir() -> PathBuf {
        Self::config_dir().join("projects")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = Config::default();
        assert!(cfg.api_key.is_none());
        assert!(cfg.model.contains("sonnet"));
        assert_eq!(cfg.max_tokens, 16384);
        assert!(!cfg.verbose);
    }

    #[test]
    fn test_config_deserialize() {
        let json = r#"{"model":"claude-opus-4-20250514","max_tokens":8192}"#;
        let cfg: Config = serde_json::from_str(json).unwrap();
        assert!(cfg.model.contains("opus"));
        assert_eq!(cfg.max_tokens, 8192);
    }

    #[test]
    fn test_config_dir_exists() {
        let dir = Config::config_dir();
        assert!(dir.to_string_lossy().contains(".claude"));
    }

    #[test]
    fn test_projects_dir() {
        let dir = Config::projects_dir();
        assert!(dir.to_string_lossy().contains("projects"));
    }

    #[test]
    fn test_merge_overrides_non_defaults() {
        let mut base = Config::default();
        let overlay = Config {
            model: "claude-opus-4-20250514".into(),
            max_tokens: 8192,
            ..Config::default()
        };
        base.merge(overlay);
        assert!(base.model.contains("opus"));
        assert_eq!(base.max_tokens, 8192);
        // api_key should remain None (overlay was also None)
        assert!(base.api_key.is_none());
    }

    #[test]
    fn test_merge_preserves_base_when_overlay_is_default() {
        let mut base = Config {
            model: "claude-opus-4-20250514".into(),
            ..Config::default()
        };
        let overlay = Config::default();
        base.merge(overlay);
        // base model should be preserved since overlay has default
        assert!(base.model.contains("opus"));
    }
}
