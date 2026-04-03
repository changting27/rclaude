//! Git worktree tools: EnterWorktree, ExitWorktree.

use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

pub struct EnterWorktreeTool;

#[async_trait]
impl Tool for EnterWorktreeTool {
    fn name(&self) -> &str {
        "EnterWorktree"
    }

    fn description(&self) -> &str {
        "Create an isolated git worktree and switch into it. \
         Use when the user explicitly asks to work in a worktree."
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Optional name for the worktree"
                }
            }
        }))
        .expect("valid schema")
    }

    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("wt-{}", &uuid::Uuid::new_v4().to_string()[..8]));

        let worktrees_dir = ctx.cwd.join(".claude").join("worktrees");
        let worktree_path = worktrees_dir.join(&name);
        let branch_name = format!("worktree/{name}");

        // Create worktree
        let output = tokio::process::Command::new("git")
            .args(["worktree", "add", "-b", &branch_name])
            .arg(&worktree_path)
            .current_dir(&ctx.cwd)
            .output()
            .await
            .map_err(|e| RclaudeError::Tool(format!("Failed to create worktree: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(ToolResult::error(format!(
                "Failed to create worktree: {stderr}"
            )));
        }

        Ok(ToolResult::text(format!(
            "Worktree created at {}\nBranch: {branch_name}\n\
             Session working directory switched to the worktree.",
            worktree_path.display()
        )))
    }
}

pub struct ExitWorktreeTool;

#[async_trait]
impl Tool for ExitWorktreeTool {
    fn name(&self) -> &str {
        "ExitWorktree"
    }

    fn description(&self) -> &str {
        "Exit a worktree session and return to the original directory."
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["keep", "remove"],
                    "description": "\"keep\" leaves worktree on disk; \"remove\" deletes it"
                },
                "discard_changes": {
                    "type": "boolean",
                    "description": "Required true when action is remove and worktree has uncommitted changes"
                }
            },
            "required": ["action"]
        }))
        .expect("valid schema")
    }

    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("keep");

        match action {
            "keep" => Ok(ToolResult::text(
                "Worktree kept on disk. Session returned to original directory.",
            )),
            "remove" => {
                // Remove worktree
                let output = tokio::process::Command::new("git")
                    .args(["worktree", "remove", "--force"])
                    .arg(ctx.cwd.to_str().unwrap_or("."))
                    .output()
                    .await;

                match output {
                    Ok(o) if o.status.success() => Ok(ToolResult::text(
                        "Worktree removed. Session returned to original directory.",
                    )),
                    Ok(o) => {
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        Ok(ToolResult::error(format!(
                            "Failed to remove worktree: {stderr}"
                        )))
                    }
                    Err(e) => Ok(ToolResult::error(format!("Failed to remove worktree: {e}"))),
                }
            }
            _ => Ok(ToolResult::error(format!("Unknown action: {action}"))),
        }
    }
}
