//! MCP WebSocket transport implementation.
//! Provides JSON-RPC communication over WebSocket connections.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};

use crate::types::*;
use rclaude_core::error::{RclaudeError, Result};

/// MCP client over WebSocket transport.
pub struct McpWebSocketClient {
    name: String,
    sender: Option<mpsc::Sender<String>>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>>,
    next_id: AtomicU64,
    tools: Vec<McpToolDef>,
}

impl McpWebSocketClient {
    /// Connect to an MCP server via WebSocket.
    pub async fn connect(name: &str, url: &str) -> Result<Self> {
        // Use tokio-tungstenite for WebSocket
        let (ws_stream, _) = tokio_tungstenite::connect_async(url).await.map_err(|e| {
            RclaudeError::Tool(format!("WebSocket connect failed for '{}': {e}", name))
        })?;

        let (write, mut read) = futures::StreamExt::split(ws_stream);
        let write = Arc::new(Mutex::new(write));

        let (tx, mut rx) = mpsc::channel::<String>(100);
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<JsonRpcResponse>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        // Writer task: send messages from channel to WebSocket
        let write_clone = write.clone();
        tokio::spawn(async move {
            use futures::SinkExt;
            use tokio_tungstenite::tungstenite::Message;
            while let Some(msg) = rx.recv().await {
                let mut w = write_clone.lock().await;
                if w.send(Message::Text(msg)).await.is_err() {
                    break;
                }
            }
        });

        // Reader task: receive messages from WebSocket and route to pending
        let pending_clone = pending.clone();
        tokio::spawn(async move {
            use futures::StreamExt;
            while let Some(Ok(msg)) = read.next().await {
                if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                    if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(&text) {
                        if let Some(id) = response.id {
                            let mut p = pending_clone.lock().await;
                            if let Some(tx) = p.remove(&id) {
                                let _ = tx.send(response);
                            }
                        }
                    }
                }
            }
        });

        let mut client = Self {
            name: name.to_string(),
            sender: Some(tx),
            pending,
            next_id: AtomicU64::new(1),
            tools: Vec::new(),
        };

        // Initialize
        let _init_result = client
            .send_request(
                "initialize",
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "rclaude", "version": env!("CARGO_PKG_VERSION") }
                }),
            )
            .await?;

        // Send initialized notification
        client
            .send_notification("notifications/initialized", serde_json::json!({}))
            .await?;

        // Fetch tools
        client.refresh_tools().await?;

        Ok(client)
    }

    async fn send_request(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let (tx, rx) = oneshot::channel();
        {
            self.pending.lock().await.insert(id, tx);
        }

        let sender = self
            .sender
            .as_ref()
            .ok_or(RclaudeError::Tool("Not connected".into()))?;
        sender
            .send(serde_json::to_string(&msg)?)
            .await
            .map_err(|e| RclaudeError::Tool(format!("Send failed: {e}")))?;

        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(resp)) => {
                if let Some(error) = resp.error {
                    Err(RclaudeError::Tool(format!("MCP error: {}", error.message)))
                } else {
                    Ok(resp.result.unwrap_or(Value::Null))
                }
            }
            Ok(Err(_)) => Err(RclaudeError::Tool("Response channel closed".into())),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                Err(RclaudeError::Tool("Request timed out".into()))
            }
        }
    }

    async fn send_notification(&self, method: &str, params: Value) -> Result<()> {
        let msg = serde_json::json!({ "jsonrpc": "2.0", "method": method, "params": params });
        let sender = self
            .sender
            .as_ref()
            .ok_or(RclaudeError::Tool("Not connected".into()))?;
        sender
            .send(serde_json::to_string(&msg)?)
            .await
            .map_err(|e| RclaudeError::Tool(format!("Send failed: {e}")))?;
        Ok(())
    }

    pub async fn refresh_tools(&mut self) -> Result<()> {
        let result = self
            .send_request("tools/list", serde_json::json!({}))
            .await?;
        if let Some(tools) = result.get("tools").and_then(|v| v.as_array()) {
            self.tools = tools
                .iter()
                .filter_map(|t| serde_json::from_value(t.clone()).ok())
                .collect();
        }
        Ok(())
    }

    pub fn tools(&self) -> &[McpToolDef] {
        &self.tools
    }
    pub fn name(&self) -> &str {
        &self.name
    }

    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<McpToolResult> {
        let result = self
            .send_request(
                "tools/call",
                serde_json::json!({
                    "name": name, "arguments": arguments
                }),
            )
            .await?;
        serde_json::from_value(result).map_err(|e| RclaudeError::Tool(format!("Parse error: {e}")))
    }

    pub async fn shutdown(&self) -> Result<()> {
        // Channel drops when client is dropped
        Ok(())
    }
}
