use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

pub struct PowerShellTool;

#[async_trait]
impl Tool for PowerShellTool {
    fn name(&self) -> &str {
        "PowerShell"
    }
    fn description(&self) -> &str {
        "Execute PowerShell commands (Windows only)."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "PowerShell command" }
            },
            "required": ["command"]
        }))
        .expect("valid schema")
    }
    fn is_available(&self, _ctx: &ToolUseContext) -> bool {
        cfg!(target_os = "windows")
    }
    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let command = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing command".into()))?;

        #[cfg(target_os = "windows")]
        {
            let output = tokio::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", command])
                .current_dir(&ctx.cwd)
                .output()
                .await?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let mut result = stdout.to_string();
            if !stderr.is_empty() {
                result.push_str(&format!("\n{stderr}"));
            }
            if output.status.success() {
                Ok(ToolResult::text(result))
            } else {
                Ok(ToolResult::error(result))
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = (command, ctx);
            Ok(ToolResult::error(
                "PowerShell is only available on Windows".to_string(),
            ))
        }
    }
}
