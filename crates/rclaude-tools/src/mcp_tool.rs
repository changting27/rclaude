//! MCPTool: dynamic wrapper for tools provided by MCP servers.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

/// MCPTool wraps a specific MCP server tool with its name, schema, and client reference.
pub struct McpToolWrapper {
    pub server_name: String,
    pub tool_name: String,
    pub full_name: String,
    pub tool_description: String,
    pub schema: ToolInputSchema,
    pub is_read_only: bool,
    /// Reference to the MCP client for actual tool calls.
    pub client: Option<Arc<rclaude_mcp::client::McpClient>>,
}

impl McpToolWrapper {
    /// Create from an MCP tool definition (matching fetchToolsForClient).
    pub fn new(
        server_name: &str,
        tool_name: &str,
        description: &str,
        input_schema: Option<Value>,
        is_read_only: bool,
        client: Option<Arc<rclaude_mcp::client::McpClient>>,
    ) -> Self {
        // Name format: mcp__<server>__<tool> (matching buildMcpToolName)
        let full_name = format!("mcp__{server_name}__{tool_name}");
        let schema: ToolInputSchema = input_schema
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_else(|| ToolInputSchema {
                schema_type: "object".to_string(),
                properties: Default::default(),
                required: vec![],
                extra: Default::default(),
            });

        Self {
            server_name: server_name.to_string(),
            tool_name: tool_name.to_string(),
            full_name,
            tool_description: if description.len() > 10_000 {
                format!("{}… [truncated]", &description[..10_000])
            } else {
                description.to_string()
            },
            schema,
            is_read_only,
            client,
        }
    }

    /// Create tools from an MCP client's discovered tools.
    pub fn from_client(client: Arc<rclaude_mcp::client::McpClient>) -> Vec<Box<dyn Tool>> {
        let server_name = client.name().to_string();
        client
            .tools()
            .iter()
            .map(|tool_def| {
                let wrapper = McpToolWrapper::new(
                    &server_name,
                    &tool_def.name,
                    tool_def.description.as_deref().unwrap_or(""),
                    tool_def.input_schema.clone(),
                    false,
                    Some(client.clone()),
                );
                Box::new(wrapper) as Box<dyn Tool>
            })
            .collect()
    }
}

#[async_trait]
impl Tool for McpToolWrapper {
    fn name(&self) -> &str {
        &self.full_name
    }
    fn display_name(&self) -> &str {
        &self.full_name
    }
    fn description(&self) -> &str {
        &self.tool_description
    }
    fn input_schema(&self) -> ToolInputSchema {
        self.schema.clone()
    }

    fn is_concurrency_safe(&self) -> bool {
        self.is_read_only
    }

    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let client = self.client.as_ref().ok_or_else(|| {
            RclaudeError::Tool(format!(
                "MCP server '{}' is not connected. Check .mcp.json configuration.",
                self.server_name
            ))
        })?;

        // Call the MCP server tool
        match client.call_tool(&self.tool_name, input).await {
            Ok(result) => {
                // Convert MCP result to ToolResult
                let text = result
                    .content
                    .iter()
                    .filter_map(|c| c.text.as_deref())
                    .collect::<Vec<_>>()
                    .join("\n");

                if result.is_error {
                    Ok(ToolResult::error(text))
                } else if text.is_empty() {
                    Ok(ToolResult::text("(MCP tool returned empty result)"))
                } else {
                    // Truncate large results (matching maxResultSizeChars: 100_000)
                    if text.len() > 100_000 {
                        Ok(ToolResult::text(format!(
                            "{}… [truncated from {} chars]",
                            &text[..100_000],
                            text.len()
                        )))
                    } else {
                        Ok(ToolResult::text(text))
                    }
                }
            }
            Err(e) => Ok(ToolResult::error(format!(
                "MCP tool '{}' on server '{}' failed: {e}",
                self.tool_name, self.server_name
            ))),
        }
    }
}
