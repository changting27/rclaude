use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{oneshot, Mutex};

use crate::types::*;
use rclaude_core::error::{RclaudeError, Result};

/// MCP client that communicates with a server over stdio.
pub struct McpClient {
    name: String,
    child: Arc<Mutex<Child>>,
    stdin: Arc<Mutex<tokio::process::ChildStdin>>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    next_id: AtomicU64,
    tools: Vec<McpToolDef>,
}

impl McpClient {
    /// Spawn an MCP server process and connect to it.
    pub async fn connect(name: &str, config: &McpServerConfig) -> Result<Self> {
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        for (k, v) in &config.env {
            cmd.env(k, v);
        }

        if let Some(ref cwd) = config.cwd {
            cmd.current_dir(cwd);
        }

        let mut child = cmd.spawn().map_err(|e| {
            RclaudeError::Tool(format!(
                "Failed to start MCP server '{}' ({}): {e}",
                name, config.command
            ))
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| RclaudeError::Tool(format!("MCP server '{name}' has no stdin")))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| RclaudeError::Tool(format!("MCP server '{name}' has no stdout")))?;

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Spawn reader task
        let pending_clone = pending.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break, // EOF
                    Ok(_) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(trimmed) {
                            if let Some(id) = resp.id {
                                let mut map = pending_clone.lock().await;
                                if let Some(tx) = map.remove(&id) {
                                    let _ = tx.send(resp);
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let mut client = Self {
            name: name.to_string(),
            child: Arc::new(Mutex::new(child)),
            stdin: Arc::new(Mutex::new(stdin)),
            pending,
            next_id: AtomicU64::new(1),
            tools: Vec::new(),
        };

        // Initialize
        client.initialize().await?;

        Ok(client)
    }

    /// Send a JSON-RPC request and wait for response.
    async fn request(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = JsonRpcRequest::new(id, method, params);
        let mut payload = serde_json::to_string(&req)?;
        payload.push('\n');

        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending.lock().await;
            map.insert(id, tx);
        }

        {
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(payload.as_bytes()).await.map_err(|e| {
                RclaudeError::Tool(format!(
                    "Failed to write to MCP server '{}': {e}",
                    self.name
                ))
            })?;
            stdin.flush().await.ok();
        }

        let resp = tokio::time::timeout(std::time::Duration::from_secs(30), rx)
            .await
            .map_err(|_| {
                RclaudeError::Timeout(format!(
                    "MCP server '{}' did not respond within 30s",
                    self.name
                ))
            })?
            .map_err(|_| {
                RclaudeError::Tool(format!(
                    "MCP server '{}' response channel dropped",
                    self.name
                ))
            })?;

        if let Some(err) = resp.error {
            return Err(RclaudeError::Tool(format!(
                "MCP error from '{}': {} ({})",
                self.name, err.message, err.code
            )));
        }

        Ok(resp.result.unwrap_or(Value::Null))
    }

    /// Initialize the MCP connection (handshake).
    async fn initialize(&mut self) -> Result<()> {
        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "roots": { "listChanged": false }
            },
            "clientInfo": {
                "name": "rclaude",
                "version": env!("CARGO_PKG_VERSION")
            }
        });

        let _result = self.request("initialize", Some(params)).await?;

        // Send initialized notification (no response expected)
        {
            let notif = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            });
            let mut payload = serde_json::to_string(&notif)?;
            payload.push('\n');
            let mut stdin = self.stdin.lock().await;
            stdin.write_all(payload.as_bytes()).await.ok();
            stdin.flush().await.ok();
        }

        // Fetch tools
        self.refresh_tools().await?;

        Ok(())
    }

    /// Refresh the list of tools from the server.
    pub async fn refresh_tools(&mut self) -> Result<()> {
        let result = self.request("tools/list", None).await?;
        if let Some(tools) = result.get("tools") {
            self.tools = serde_json::from_value(tools.clone()).unwrap_or_default();
        }
        Ok(())
    }

    /// Get the list of available tools.
    pub fn tools(&self) -> &[McpToolDef] {
        &self.tools
    }

    /// Get the server name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Call a tool on the MCP server.
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<McpToolResult> {
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments
        });
        let result = self.request("tools/call", Some(params)).await?;
        serde_json::from_value(result)
            .map_err(|e| RclaudeError::Tool(format!("Failed to parse tool result: {e}")))
    }

    /// List resources from the MCP server.
    pub async fn list_resources(&self) -> Result<Vec<McpResource>> {
        let result = self.request("resources/list", None).await?;
        if let Some(resources) = result.get("resources") {
            Ok(serde_json::from_value(resources.clone()).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }

    /// Read a resource from the MCP server.
    pub async fn read_resource(&self, uri: &str) -> Result<Value> {
        let params = serde_json::json!({ "uri": uri });
        self.request("resources/read", Some(params)).await
    }

    /// Shut down the MCP server gracefully.
    pub async fn shutdown(&self) -> Result<()> {
        let _ = self.request("shutdown", None).await;
        let mut child = self.child.lock().await;
        child.kill().await.ok();
        Ok(())
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // Best-effort kill on drop
    }
}
