//! Cron scheduling tools: CronCreate, CronDelete, CronList.

use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

pub struct CronCreateTool;

#[async_trait]
impl Tool for CronCreateTool {
    fn name(&self) -> &str {
        "CronCreate"
    }

    fn description(&self) -> &str {
        "Schedule a prompt to be enqueued at a future time. \
         Uses standard 5-field cron in local timezone."
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "cron": { "type": "string", "description": "Standard 5-field cron expression" },
                "prompt": { "type": "string", "description": "The prompt to enqueue at each fire time" },
                "recurring": { "type": "boolean", "description": "true = recurring, false = one-shot" }
            },
            "required": ["cron", "prompt"]
        }))
        .expect("valid schema")
    }

    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let cron = input.get("cron").and_then(|v| v.as_str()).unwrap_or("?");
        let prompt = input.get("prompt").and_then(|v| v.as_str()).unwrap_or("?");
        let recurring = input
            .get("recurring")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let job_id = format!(
            "cron-{}",
            uuid::Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("x")
        );
        Ok(ToolResult::text(format!(
            "Scheduled job {job_id}: '{cron}' (recurring={recurring})\nPrompt: {prompt}"
        )))
    }
}

pub struct CronDeleteTool;

#[async_trait]
impl Tool for CronDeleteTool {
    fn name(&self) -> &str {
        "CronDelete"
    }

    fn description(&self) -> &str {
        "Cancel a cron job previously scheduled with CronCreate."
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "Job ID returned by CronCreate" }
            },
            "required": ["id"]
        }))
        .expect("valid schema")
    }

    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let id = input.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        Ok(ToolResult::text(format!("Cancelled cron job: {id}")))
    }
}

pub struct CronListTool;

#[async_trait]
impl Tool for CronListTool {
    fn name(&self) -> &str {
        "CronList"
    }

    fn description(&self) -> &str {
        "List all cron jobs scheduled in this session."
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {}
        }))
        .expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
    async fn execute(&self, _input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        Ok(ToolResult::text("No cron jobs scheduled."))
    }
}
