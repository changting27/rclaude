//! ReadMcpResource: read a specific resource from an MCP server.

use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

pub struct ReadMcpResourceTool;

#[async_trait]
impl Tool for ReadMcpResourceTool {
    fn name(&self) -> &str {
        "ReadMcpResource"
    }
    fn description(&self) -> &str {
        "Read a specific resource from a connected MCP server by URI."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "server_name": { "type": "string", "description": "MCP server name" },
                "uri": { "type": "string", "description": "Resource URI to read" }
            },
            "required": ["server_name", "uri"]
        }))
        .expect("valid schema")
    }
    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let server_name = input
            .get("server_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let uri = input.get("uri").and_then(|v| v.as_str()).unwrap_or("");

        if server_name.is_empty() || uri.is_empty() {
            return Ok(ToolResult::error("Both server_name and uri are required."));
        }

        // Try to connect and read
        let config = rclaude_mcp::config::load_mcp_config(&_ctx.cwd)?;
        let server_config = match config.mcp_servers.get(server_name) {
            Some(c) => c,
            None => {
                return Ok(ToolResult::error(format!(
                    "MCP server '{server_name}' not found in config."
                )));
            }
        };

        match rclaude_mcp::client::McpClient::connect(server_name, server_config).await {
            Ok(client) => match client.read_resource(uri).await {
                Ok(result) => Ok(ToolResult::text(
                    serde_json::to_string_pretty(&result).unwrap_or_default(),
                )),
                Err(e) => Ok(ToolResult::error(format!("Failed to read resource: {e}"))),
            },
            Err(e) => Ok(ToolResult::error(format!(
                "Failed to connect to MCP server '{server_name}': {e}"
            ))),
        }
    }
}
