//! McpAuthTool: trigger OAuth authentication for an MCP server.

use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

pub struct McpAuthTool;

#[async_trait]
impl Tool for McpAuthTool {
    fn name(&self) -> &str {
        "McpAuth"
    }
    fn description(&self) -> &str {
        "Authenticate with an MCP server that requires OAuth."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "server_name": { "type": "string", "description": "MCP server name from config" }
            },
            "required": ["server_name"]
        }))
        .expect("valid schema")
    }
    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let server_name = input
            .get("server_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let config = rclaude_mcp::config::load_mcp_config(&ctx.cwd)?;
        match config.mcp_servers.get(server_name) {
            Some(_server) => Ok(ToolResult::text(format!(
                "MCP server '{server_name}' found. OAuth authentication is not yet supported \
                 for this server type. Use API key authentication via env vars in .mcp.json."
            ))),
            None => Ok(ToolResult::error(format!(
                "MCP server '{server_name}' not found in config."
            ))),
        }
    }
}
