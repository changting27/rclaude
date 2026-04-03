//! ListMcpResources: list resources from connected MCP servers.

use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

pub struct ListMcpResourcesTool;

#[async_trait]
impl Tool for ListMcpResourcesTool {
    fn name(&self) -> &str {
        "ListMcpResources"
    }
    fn description(&self) -> &str {
        "List available resources from connected MCP servers."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "server": { "type": "string", "description": "MCP server name (optional)" }
            }
        }))
        .expect("valid schema")
    }
    async fn execute(&self, _input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        // Load MCP config and list resources
        let config = rclaude_mcp::config::load_mcp_config(&_ctx.cwd)?;
        if config.mcp_servers.is_empty() {
            return Ok(ToolResult::text(
                "No MCP servers configured. Add servers to .mcp.json.",
            ));
        }

        let mut output = String::from("Configured MCP servers:\n");
        for (name, server) in &config.mcp_servers {
            output.push_str(&format!(
                "  - {name}: {} {}\n",
                server.command,
                server.args.join(" ")
            ));
        }
        output.push_str("\nConnect to a server to list its resources.");
        Ok(ToolResult::text(output))
    }
}
