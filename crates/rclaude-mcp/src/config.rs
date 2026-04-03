use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::types::McpServerConfig;
use rclaude_core::error::Result;

/// MCP configuration file structure (from .mcp.json or claude_desktop_config.json).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfig {
    #[serde(default, rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

/// Load MCP configuration from standard locations.
pub fn load_mcp_config(cwd: &Path) -> Result<McpConfig> {
    let mut merged = McpConfig::default();

    // 1. Traverse parent directories for .mcp.json
    let mut dir = cwd.to_path_buf();
    loop {
        let mcp_json = dir.join(".mcp.json");
        if mcp_json.exists() {
            if let Ok(content) = std::fs::read_to_string(&mcp_json) {
                if let Ok(config) = serde_json::from_str::<McpConfig>(&content) {
                    // Merge: child overrides parent
                    for (name, server) in config.mcp_servers {
                        merged.mcp_servers.entry(name).or_insert(server);
                    }
                }
            }
        }
        if !dir.pop() {
            break;
        }
    }

    // 2. User-level MCP config (~/.claude/mcp.json)
    let home_config = dirs::home_dir()
        .map(|h| h.join(".claude").join("mcp.json"))
        .unwrap_or_default();
    if home_config.exists() {
        if let Ok(content) = std::fs::read_to_string(&home_config) {
            if let Ok(config) = serde_json::from_str::<McpConfig>(&content) {
                for (name, server) in config.mcp_servers {
                    merged.mcp_servers.entry(name).or_insert(server);
                }
            }
        }
    }

    // 3. MCP servers from settings.json (mcpServers key)
    let settings_path = cwd.join(".claude/settings.json");
    if settings_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&settings_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(servers) = val.get("mcpServers").and_then(|v| v.as_object()) {
                    for (name, server_val) in servers {
                        if let Ok(server) =
                            serde_json::from_value::<McpServerConfig>(server_val.clone())
                        {
                            merged.mcp_servers.entry(name.clone()).or_insert(server);
                        }
                    }
                }
            }
        }
    }

    Ok(merged)
}

/// Expand environment variables in a string: ${VAR} and $VAR patterns.
pub fn expand_env_vars(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '$' && i + 1 < chars.len() {
            if chars[i + 1] == '{' {
                // ${VAR} pattern
                if let Some(end) = chars[i + 2..].iter().position(|&c| c == '}') {
                    let var_name: String = chars[i + 2..i + 2 + end].iter().collect();
                    result.push_str(&std::env::var(&var_name).unwrap_or_default());
                    i += end + 3;
                    continue;
                }
            } else if chars[i + 1].is_ascii_alphanumeric() || chars[i + 1] == '_' {
                // $VAR pattern
                let start = i + 1;
                let mut end = start;
                while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '_')
                {
                    end += 1;
                }
                let var_name: String = chars[start..end].iter().collect();
                result.push_str(&std::env::var(&var_name).unwrap_or_default());
                i = end;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
}

/// Expand env vars in all string fields of an MCP server config.
pub fn expand_config_env_vars(config: &mut McpConfig) {
    for server in config.mcp_servers.values_mut() {
        server.command = expand_env_vars(&server.command);
        server.args = server.args.iter().map(|a| expand_env_vars(a)).collect();
        let expanded_env: HashMap<String, String> = server
            .env
            .iter()
            .map(|(k, v)| (k.clone(), expand_env_vars(v)))
            .collect();
        server.env = expanded_env;
        if let Some(ref url) = server.url {
            server.url = Some(expand_env_vars(url));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_env_vars_braces() {
        std::env::set_var("TEST_RCLAUDE_VAR", "hello");
        assert_eq!(expand_env_vars("${TEST_RCLAUDE_VAR}"), "hello");
        assert_eq!(
            expand_env_vars("pre-${TEST_RCLAUDE_VAR}-post"),
            "pre-hello-post"
        );
        std::env::remove_var("TEST_RCLAUDE_VAR");
    }

    #[test]
    fn test_expand_env_vars_bare() {
        std::env::set_var("TEST_RCLAUDE_VAR2", "world");
        assert_eq!(expand_env_vars("$TEST_RCLAUDE_VAR2"), "world");
        std::env::remove_var("TEST_RCLAUDE_VAR2");
    }

    #[test]
    fn test_expand_env_vars_missing() {
        assert_eq!(expand_env_vars("${NONEXISTENT_VAR_XYZ}"), "");
    }
}
