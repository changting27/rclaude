use async_trait::async_trait;
use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};
use serde_json::{json, Value};

pub struct PushNotificationTool;

#[async_trait]
impl Tool for PushNotificationTool {
    fn name(&self) -> &str {
        "PushNotification"
    }
    fn description(&self) -> &str {
        "Send a push notification to the user's device."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({"type": "object", "properties": {
            "title": {"type": "string"}, "body": {"type": "string"}
        }, "required": ["title", "body"]}))
        .expect("valid schema")
    }
    fn is_available(&self, _ctx: &ToolUseContext) -> bool {
        false
    }
    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let title = input.get("title").and_then(|v| v.as_str()).unwrap_or("?");
        Ok(ToolResult::text(format!("Push notification: {title}")))
    }
}
