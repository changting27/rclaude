use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::Result;
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

pub struct BriefTool;

#[async_trait]
impl Tool for BriefTool {
    fn name(&self) -> &str {
        "Brief"
    }
    fn description(&self) -> &str {
        "Attach files or screenshots to the conversation."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "content": { "type": "string", "description": "Content to attach" },
                "file_path": { "type": "string", "description": "File path to attach" }
            }
        }))
        .expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
            let full = if std::path::Path::new(path).is_absolute() {
                std::path::PathBuf::from(path)
            } else {
                ctx.cwd.join(path)
            };
            if full.exists() {
                let size = tokio::fs::metadata(&full).await?.len();
                Ok(ToolResult::text(format!(
                    "Attached: {} ({} bytes)",
                    full.display(),
                    size
                )))
            } else {
                Ok(ToolResult::error(format!("File not found: {path}")))
            }
        } else if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
            Ok(ToolResult::text(format!(
                "Attached {} bytes of content",
                content.len()
            )))
        } else {
            Ok(ToolResult::error(
                "Provide file_path or content".to_string(),
            ))
        }
    }
}
