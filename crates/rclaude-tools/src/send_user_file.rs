use async_trait::async_trait;
use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};
use serde_json::{json, Value};

pub struct SendUserFileTool;

#[async_trait]
impl Tool for SendUserFileTool {
    fn name(&self) -> &str {
        "SendUserFile"
    }
    fn description(&self) -> &str {
        "Send a file to the user's device."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({"type": "object", "properties": {
            "file_path": {"type": "string"}
        }, "required": ["file_path"]}))
        .expect("valid schema")
    }
    fn is_available(&self, _ctx: &ToolUseContext) -> bool {
        false
    }
    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let path = input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        Ok(ToolResult::text(format!("File sent: {path}")))
    }
}
