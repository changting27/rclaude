use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

pub struct WorkflowTool;

#[async_trait]
impl Tool for WorkflowTool {
    fn name(&self) -> &str {
        "Workflow"
    }
    fn description(&self) -> &str {
        "Execute a predefined workflow script."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "name": { "type": "string", "description": "Workflow name" },
                "args": { "type": "object", "description": "Workflow arguments" }
            },
            "required": ["name"]
        }))
        .expect("valid schema")
    }
    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let name = input.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        Ok(ToolResult::text(format!(
            "Workflow '{name}' — workflow scripts not yet loaded"
        )))
    }
}
