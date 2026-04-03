use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

pub struct MonitorTool;

#[async_trait]
impl Tool for MonitorTool {
    fn name(&self) -> &str {
        "Monitor"
    }
    fn description(&self) -> &str {
        "Monitor a running process or resource for changes."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "target": { "type": "string", "description": "What to monitor" },
                "interval_ms": { "type": "number", "description": "Poll interval in ms" }
            },
            "required": ["target"]
        }))
        .expect("valid schema")
    }
    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let target = input.get("target").and_then(|v| v.as_str()).unwrap_or("?");
        Ok(ToolResult::text(format!(
            "Monitoring '{target}' — background monitoring not yet implemented"
        )))
    }
}
