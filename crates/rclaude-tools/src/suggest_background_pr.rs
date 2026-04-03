use async_trait::async_trait;
use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};
use serde_json::{json, Value};

pub struct SuggestBackgroundPRTool;

#[async_trait]
impl Tool for SuggestBackgroundPRTool {
    fn name(&self) -> &str {
        "SuggestBackgroundPR"
    }
    fn description(&self) -> &str {
        "Suggest creating a background PR for non-blocking changes."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({"type": "object", "properties": {
            "title": {"type": "string"}, "description": {"type": "string"}
        }, "required": ["title"]}))
        .expect("valid schema")
    }
    fn is_available(&self, _ctx: &ToolUseContext) -> bool {
        false
    } // feature-gated
    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let title = input.get("title").and_then(|v| v.as_str()).unwrap_or("?");
        Ok(ToolResult::text(format!(
            "Background PR suggested: {title}"
        )))
    }
}
