//! Q08: MCP Connection Manager with multi-transport support.

use std::collections::HashMap;

use crate::client::McpClient;
use crate::config::{load_mcp_config, McpConfig};
use crate::sse::McpSseClient;
use crate::types::{McpServerConfig, McpToolDef, McpToolResult};
use rclaude_core::error::{RclaudeError, Result};

/// Unified MCP connection — wraps different transport types.
pub enum McpConnection {
    Stdio(McpClient),
    Sse(McpSseClient),
}

impl McpConnection {
    pub fn name(&self) -> &str {
        match self {
            Self::Stdio(c) => c.name(),
            Self::Sse(c) => c.name(),
        }
    }

    pub fn tools(&self) -> &[McpToolDef] {
        match self {
            Self::Stdio(c) => c.tools(),
            Self::Sse(c) => c.tools(),
        }
    }

    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<McpToolResult> {
        match self {
            Self::Stdio(c) => c.call_tool(name, arguments).await,
            Self::Sse(c) => c.call_tool(name, arguments).await,
        }
    }

    pub async fn shutdown(&self) -> Result<()> {
        match self {
            Self::Stdio(c) => c.shutdown().await,
            Self::Sse(c) => c.shutdown().await,
        }
    }
}

/// Manages multiple MCP server connections across transports.
pub struct McpConnectionManager {
    connections: HashMap<String, McpConnection>,
    config: McpConfig,
}

impl McpConnectionManager {
    /// Create a new manager and connect to all configured servers.
    pub async fn from_config(cwd: &std::path::Path) -> Result<Self> {
        let mut config = load_mcp_config(cwd)?;
        // Expand env vars in all server configs
        crate::config::expand_config_env_vars(&mut config);
        let mut connections = HashMap::new();

        for (name, server_config) in &config.mcp_servers {
            // Check server approval before connecting
            if !crate::approval::check_server_approved(name, &server_config.command) {
                tracing::info!("MCP '{}' not approved, skipping", name);
                continue;
            }
            match connect_server(name, server_config).await {
                Ok(conn) => {
                    tracing::info!(
                        "MCP '{}' connected via {} ({} tools)",
                        name,
                        server_config.transport,
                        conn.tools().len()
                    );
                    connections.insert(name.clone(), conn);
                }
                Err(e) => {
                    tracing::warn!("Failed to connect MCP '{}': {}", name, e);
                }
            }
        }

        Ok(Self {
            connections,
            config,
        })
    }

    pub fn get(&self, name: &str) -> Option<&McpConnection> {
        self.connections.get(name)
    }

    pub fn all_tools(&self) -> Vec<(&str, &McpToolDef)> {
        self.connections
            .iter()
            .flat_map(|(name, conn)| conn.tools().iter().map(move |t| (name.as_str(), t)))
            .collect()
    }

    pub async fn reconnect(&mut self, name: &str) -> Result<()> {
        if let Some(old) = self.connections.remove(name) {
            let _ = old.shutdown().await;
        }
        let config = self
            .config
            .mcp_servers
            .get(name)
            .ok_or_else(|| RclaudeError::Config(format!("No MCP config for '{name}'")))?;
        let conn = connect_server(name, config).await?;
        self.connections.insert(name.to_string(), conn);
        Ok(())
    }

    pub async fn disconnect(&mut self, name: &str) {
        if let Some(conn) = self.connections.remove(name) {
            let _ = conn.shutdown().await;
        }
    }

    pub async fn shutdown_all(&mut self) {
        let names: Vec<String> = self.connections.keys().cloned().collect();
        for name in names {
            self.disconnect(&name).await;
        }
    }

    pub fn connected_count(&self) -> usize {
        self.connections.len()
    }

    pub fn server_names(&self) -> Vec<&str> {
        self.connections.keys().map(|s| s.as_str()).collect()
    }
}

/// Connect to a server using the appropriate transport.
async fn connect_server(name: &str, config: &McpServerConfig) -> Result<McpConnection> {
    match config.transport.as_str() {
        "sse" | "http" => {
            let url = config
                .url
                .as_deref()
                .ok_or_else(|| RclaudeError::Config(format!("SSE server '{name}' needs a url")))?;

            // Check if OAuth is needed (server has oauth config or returns 401)
            if config.oauth.is_some() {
                let server_key = crate::oauth::get_server_key(name, url);
                let _tokens = match crate::oauth::get_stored_tokens(&server_key) {
                    Some(t) => t,
                    None => {
                        // Need to perform OAuth flow
                        let callback_port = config
                            .oauth
                            .as_ref()
                            .and_then(|o| o.get("callbackPort"))
                            .and_then(|v| v.as_u64())
                            .map(|p| p as u16);
                        crate::oauth::perform_oauth_flow(name, url, callback_port).await?
                    }
                };
                // TODO: pass token to SSE client headers
            }

            let client = McpSseClient::connect(name, url).await?;
            Ok(McpConnection::Sse(client))
        }
        _ => {
            // Default: stdio
            let client = McpClient::connect(name, config).await?;
            Ok(McpConnection::Stdio(client))
        }
    }
}
