//! Q08: SSE (Server-Sent Events) transport for MCP.
//! Connects to an MCP server via HTTP SSE endpoint.

use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::types::*;
use rclaude_core::error::{RclaudeError, Result};

/// MCP client using SSE transport.
pub struct McpSseClient {
    name: String,
    base_url: String,
    http: reqwest::Client,
    next_id: AtomicU64,
    tools: Vec<McpToolDef>,
}

impl McpSseClient {
    /// Connect to an MCP server via SSE.
    pub async fn connect(name: &str, url: &str) -> Result<Self> {
        let http = reqwest::Client::new();

        let mut client = Self {
            name: name.to_string(),
            base_url: url.trim_end_matches('/').to_string(),
            http,
            next_id: AtomicU64::new(1),
            tools: Vec::new(),
        };

        client.initialize().await?;
        Ok(client)
    }

    /// Send a JSON-RPC request via HTTP POST.
    async fn request(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req = JsonRpcRequest::new(id, method, params);

        let resp = self
            .http
            .post(format!("{}/rpc", self.base_url))
            .json(&req)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| {
                RclaudeError::Tool(format!("SSE request to '{}' failed: {e}", self.name))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(RclaudeError::Tool(format!(
                "SSE server '{}' returned {status}: {body}",
                self.name
            )));
        }

        let rpc_resp: JsonRpcResponse = resp.json().await.map_err(|e| {
            RclaudeError::Tool(format!(
                "Failed to parse SSE response from '{}': {e}",
                self.name
            ))
        })?;

        if let Some(err) = rpc_resp.error {
            return Err(RclaudeError::Tool(format!(
                "MCP error from '{}': {} ({})",
                self.name, err.message, err.code
            )));
        }

        Ok(rpc_resp.result.unwrap_or(Value::Null))
    }

    async fn initialize(&mut self) -> Result<()> {
        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "roots": { "listChanged": false } },
            "clientInfo": { "name": "rclaude", "version": env!("CARGO_PKG_VERSION") }
        });
        let _result = self.request("initialize", Some(params)).await?;
        self.refresh_tools().await?;
        Ok(())
    }

    pub async fn refresh_tools(&mut self) -> Result<()> {
        let result = self.request("tools/list", None).await?;
        if let Some(tools) = result.get("tools") {
            self.tools = serde_json::from_value(tools.clone()).unwrap_or_default();
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
        let params = serde_json::json!({ "name": name, "arguments": arguments });
        let result = self.request("tools/call", Some(params)).await?;
        serde_json::from_value(result)
            .map_err(|e| RclaudeError::Tool(format!("Failed to parse tool result: {e}")))
    }

    pub async fn list_resources(&self) -> Result<Vec<McpResource>> {
        let result = self.request("resources/list", None).await?;
        if let Some(resources) = result.get("resources") {
            Ok(serde_json::from_value(resources.clone()).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn read_resource(&self, uri: &str) -> Result<Value> {
        let params = serde_json::json!({ "uri": uri });
        self.request("resources/read", Some(params)).await
    }

    pub async fn shutdown(&self) -> Result<()> {
        let _ = self.request("shutdown", None).await;
        Ok(())
    }
}
