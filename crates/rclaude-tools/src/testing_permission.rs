use async_trait::async_trait;
use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};
use serde_json::{json, Value};

/// Testing-only tool for permission system validation.
pub struct TestingPermissionTool;

#[async_trait]
impl Tool for TestingPermissionTool {
    fn name(&self) -> &str {
        "TestingPermission"
    }
    fn description(&self) -> &str {
        "Internal testing tool for permission checks."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({"type": "object", "properties": {}})).expect("valid schema")
    }
    fn is_available(&self, _ctx: &ToolUseContext) -> bool {
        std::env::var("RCLAUDE_TESTING").is_ok()
    }
    async fn execute(&self, _input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        Ok(ToolResult::text("Permission test passed"))
    }
}
