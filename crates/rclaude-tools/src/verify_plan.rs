use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

pub struct VerifyPlanExecutionTool;

#[async_trait]
impl Tool for VerifyPlanExecutionTool {
    fn name(&self) -> &str {
        "VerifyPlanExecution"
    }
    fn description(&self) -> &str {
        "Verify that a plan was executed correctly."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({ "type": "object", "properties": {} })).expect("valid schema")
    }
    async fn execute(&self, _input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        Ok(ToolResult::text(
            "Plan verification: check completed tasks against plan.",
        ))
    }
}
