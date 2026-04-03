//! SleepTool: pause execution for a specified duration.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

pub struct SleepTool;

#[async_trait]
impl Tool for SleepTool {
    fn name(&self) -> &str {
        "Sleep"
    }
    fn description(&self) -> &str {
        "Pause execution for a specified number of seconds (max 300)."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "seconds": { "type": "number", "description": "Duration in seconds (max 300)" }
            },
            "required": ["seconds"]
        }))
        .expect("valid schema")
    }
    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let seconds = input
            .get("seconds")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0)
            .clamp(0.0, 300.0);
        tokio::time::sleep(Duration::from_secs_f64(seconds)).await;
        Ok(ToolResult::text(format!("Slept for {seconds:.1} seconds.")))
    }
}
