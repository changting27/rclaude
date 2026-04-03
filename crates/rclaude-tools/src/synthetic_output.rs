use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

/// Internal tool for injecting synthetic output. Hidden from LLM.
pub struct SyntheticOutputTool;

#[async_trait]
impl Tool for SyntheticOutputTool {
    fn name(&self) -> &str {
        "SyntheticOutput"
    }
    fn description(&self) -> &str {
        "Internal: inject synthetic output."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": { "output": { "type": "string" } },
            "required": ["output"]
        }))
        .expect("valid schema")
    }
    fn is_available(&self, _ctx: &ToolUseContext) -> bool {
        false
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let output = input.get("output").and_then(|v| v.as_str()).unwrap_or("");
        Ok(ToolResult::text(output.to_string()))
    }
}
