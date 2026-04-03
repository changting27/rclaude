use async_trait::async_trait;
use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};
use serde_json::{json, Value};

pub struct SubscribePRTool;

#[async_trait]
impl Tool for SubscribePRTool {
    fn name(&self) -> &str {
        "SubscribePR"
    }
    fn description(&self) -> &str {
        "Subscribe to a PR for webhook notifications."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({"type": "object", "properties": {
            "pr_url": {"type": "string"}
        }, "required": ["pr_url"]}))
        .expect("valid schema")
    }
    fn is_available(&self, _ctx: &ToolUseContext) -> bool {
        false
    }
    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let url = input.get("pr_url").and_then(|v| v.as_str()).unwrap_or("?");
        Ok(ToolResult::text(format!("Subscribed to PR: {url}")))
    }
}
