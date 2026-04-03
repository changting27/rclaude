//! Authentication system: API key management from multiple sources.
//! Handles API key management from multiple sources.

use std::path::PathBuf;

/// Source of the API key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiKeySource {
    EnvVar,        // ANTHROPIC_API_KEY
    ConfigFile,    // ~/.claude/settings.json
    ProjectConfig, // .claude/settings.json
    ApiKeyHelper,  // External key helper command
    OAuthToken,    // OAuth flow
    None,
}

/// API key with its source.
#[derive(Debug, Clone)]
pub struct ApiKeyWithSource {
    pub key: Option<String>,
    pub source: ApiKeySource,
}

/// Get the Anthropic API key from all sources (matching getAnthropicApiKeyWithSource).
/// Priority: env var → config file → project config → key helper
pub fn get_api_key_with_source() -> ApiKeyWithSource {
    // 1. Environment variable (highest priority)
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        if !key.is_empty() {
            return ApiKeyWithSource {
                key: Some(key),
                source: ApiKeySource::EnvVar,
            };
        }
    }

    // 2. Credentials file (~/.claude/.credentials.json — claude's secure storage)
    if let Some(key) = load_key_from_credentials() {
        return ApiKeyWithSource {
            key: Some(key),
            source: ApiKeySource::ConfigFile,
        };
    }

    // 3. Global config file (~/.claude/settings.json)
    if let Some(key) = load_key_from_config(&global_config_path()) {
        return ApiKeyWithSource {
            key: Some(key),
            source: ApiKeySource::ConfigFile,
        };
    }

    // 4. ~/.claude.json (OAuth token / legacy config)
    if let Some(key) = load_key_from_claude_json() {
        return ApiKeyWithSource {
            key: Some(key),
            source: ApiKeySource::OAuthToken,
        };
    }

    // 5. Project config (.claude/settings.json)
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(key) = load_key_from_config(&cwd.join(".claude/settings.json")) {
            return ApiKeyWithSource {
                key: Some(key),
                source: ApiKeySource::ProjectConfig,
            };
        }
    }

    // 6. API key helper (external command)
    if let Ok(helper) = std::env::var("CLAUDE_API_KEY_HELPER") {
        if let Some(key) = run_key_helper(&helper) {
            return ApiKeyWithSource {
                key: Some(key),
                source: ApiKeySource::ApiKeyHelper,
            };
        }
    }

    ApiKeyWithSource {
        key: None,
        source: ApiKeySource::None,
    }
}

/// Get just the API key (convenience).
pub fn get_api_key() -> Option<String> {
    get_api_key_with_source().key
}

/// Check if any API key auth is configured.
pub fn has_api_key_auth() -> bool {
    get_api_key_with_source().source != ApiKeySource::None
}

/// Save an API key to the global config file.
pub fn save_api_key(api_key: &str) -> Result<(), String> {
    let config_dir = global_config_dir();
    std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;

    let config_path = config_dir.join("settings.json");
    let mut settings: serde_json::Value = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({}));

    settings["apiKey"] = serde_json::Value::String(api_key.to_string());

    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&config_path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Remove the API key from the global config file.
pub fn remove_api_key() -> Result<(), String> {
    let config_path = global_config_dir().join("settings.json");
    if !config_path.exists() {
        return Ok(());
    }

    let mut settings: serde_json::Value = std::fs::read_to_string(&config_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(serde_json::json!({}));

    if let Some(obj) = settings.as_object_mut() {
        obj.remove("apiKey");
    }

    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&config_path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Validate an API key format.
pub fn validate_api_key(key: &str) -> bool {
    key.starts_with("sk-ant-") && key.len() > 20
}

// ── Helpers ──

fn global_config_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".claude")
}

fn global_config_path() -> PathBuf {
    global_config_dir().join("settings.json")
}

fn load_key_from_config(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    let settings: serde_json::Value = serde_json::from_str(&content).ok()?;
    settings
        .get("apiKey")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// C01: Read API key from ~/.claude/.credentials.json (claude's plaintext secure storage).
fn load_key_from_credentials() -> Option<String> {
    let path = global_config_dir().join(".credentials.json");
    let content = std::fs::read_to_string(path).ok()?;
    let creds: serde_json::Value = serde_json::from_str(&content).ok()?;
    creds
        .get("anthropicApiKey")
        .or_else(|| creds.get("apiKey"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// C01: Read API key from ~/.claude.json (OAuth token / legacy global config).
fn load_key_from_claude_json() -> Option<String> {
    let path = dirs::home_dir()?.join(".claude.json");
    let content = std::fs::read_to_string(path).ok()?;
    let config: serde_json::Value = serde_json::from_str(&content).ok()?;
    // Try oauthToken first, then apiKey
    config
        .get("oauthToken")
        .or_else(|| config.get("apiKey"))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn run_key_helper(command: &str) -> Option<String> {
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let key = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_api_key() {
        assert!(validate_api_key("sk-ant-api03-abcdefghijklmnopqrstuvwxyz"));
        assert!(!validate_api_key("invalid-key"));
        assert!(!validate_api_key("sk-ant-short"));
    }

    #[test]
    fn test_global_config_dir() {
        let dir = global_config_dir();
        assert!(dir.to_string_lossy().contains(".claude"));
    }
}
