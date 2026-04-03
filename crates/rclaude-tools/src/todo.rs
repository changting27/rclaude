//! TodoWriteTool: manage a todo/task list file.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

const DESCRIPTION: &str = "Write and manage a todo list for tracking tasks. \
Supports adding, completing, and removing items from a structured todo file.";

pub struct TodoWriteTool;

fn todo_path(cwd: &std::path::Path) -> PathBuf {
    // Use ~/.claude/todos/ directory (matching claude's global todos)
    let dir = rclaude_core::config::Config::config_dir().join("todos");
    let _ = std::fs::create_dir_all(&dir);
    let project_hash: String = cwd
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    dir.join(format!(
        "{}.md",
        &project_hash[..project_hash.len().min(40)]
    ))
}

#[async_trait]
impl Tool for TodoWriteTool {
    fn name(&self) -> &str {
        "TodoWrite"
    }
    fn description(&self) -> &str {
        DESCRIPTION
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "complete", "remove", "list"],
                    "description": "Action to perform"
                },
                "content": {
                    "type": "string",
                    "description": "Todo item content (for add) or index (for complete/remove)"
                }
            },
            "required": ["action"]
        }))
        .expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        false
    }

    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing action".into()))?;
        let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let path = todo_path(&ctx.cwd);

        match action {
            "add" => {
                if content.is_empty() {
                    return Err(RclaudeError::Tool("Content required for add".into()));
                }
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                let mut existing = tokio::fs::read_to_string(&path).await.unwrap_or_default();
                existing.push_str(&format!("- [ ] {content}\n"));
                tokio::fs::write(&path, &existing).await?;
                Ok(ToolResult::text(format!("Added: {content}")))
            }
            "complete" => {
                let idx: usize = content
                    .parse()
                    .map_err(|_| RclaudeError::Tool("Invalid index".into()))?;
                let existing = tokio::fs::read_to_string(&path).await.unwrap_or_default();
                let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();
                if idx == 0 || idx > lines.len() {
                    return Ok(ToolResult::error(format!(
                        "Index {idx} out of range (1-{})",
                        lines.len()
                    )));
                }
                lines[idx - 1] = lines[idx - 1].replace("- [ ]", "- [x]");
                tokio::fs::write(&path, lines.join("\n") + "\n").await?;
                Ok(ToolResult::text(format!("Completed item {idx}")))
            }
            "remove" => {
                let idx: usize = content
                    .parse()
                    .map_err(|_| RclaudeError::Tool("Invalid index".into()))?;
                let existing = tokio::fs::read_to_string(&path).await.unwrap_or_default();
                let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();
                if idx == 0 || idx > lines.len() {
                    return Ok(ToolResult::error(format!("Index {idx} out of range")));
                }
                lines.remove(idx - 1);
                tokio::fs::write(&path, lines.join("\n") + "\n").await?;
                Ok(ToolResult::text(format!("Removed item {idx}")))
            }
            "list" => {
                let existing = tokio::fs::read_to_string(&path).await.unwrap_or_default();
                if existing.trim().is_empty() {
                    Ok(ToolResult::text("No todos found."))
                } else {
                    let numbered: String = existing
                        .lines()
                        .enumerate()
                        .map(|(i, l)| format!("{}: {l}", i + 1))
                        .collect::<Vec<_>>()
                        .join("\n");
                    Ok(ToolResult::text(numbered))
                }
            }
            _ => Err(RclaudeError::Tool(format!("Unknown action: {action}"))),
        }
    }
}
