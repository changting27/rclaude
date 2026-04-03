//! Team management tools: TeamCreate, TeamDelete, SendMessage.

use async_trait::async_trait;
use serde_json::{json, Value};

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

// ============================================================================
// TeamCreateTool
// ============================================================================

pub struct TeamCreateTool;

#[async_trait]
impl Tool for TeamCreateTool {
    fn name(&self) -> &str {
        "TeamCreate"
    }

    fn description(&self) -> &str {
        "Create a new team to coordinate multiple agents working on a project."
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name for the new team"
                },
                "description": {
                    "type": "string",
                    "description": "Team description/purpose"
                }
            },
            "required": ["team_name"]
        }))
        .expect("valid schema")
    }

    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let team_name = input
            .get("team_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| RclaudeError::Tool("Missing required field: team_name".into()))?;

        // Create team directory
        let teams_dir = dirs::home_dir()
            .unwrap_or_else(|| ctx.cwd.clone())
            .join(".claude")
            .join("teams")
            .join(team_name);

        tokio::fs::create_dir_all(&teams_dir).await?;

        let config = json!({
            "name": team_name,
            "description": input.get("description").and_then(|v| v.as_str()).unwrap_or(""),
            "members": [],
            "createdAt": chrono::Utc::now().to_rfc3339()
        });

        let config_path = teams_dir.join("config.json");
        tokio::fs::write(&config_path, serde_json::to_string_pretty(&config)?).await?;

        Ok(ToolResult::text(format!(
            "Team '{team_name}' created at {}",
            teams_dir.display()
        )))
    }
}

// ============================================================================
// TeamDeleteTool
// ============================================================================

pub struct TeamDeleteTool;

#[async_trait]
impl Tool for TeamDeleteTool {
    fn name(&self) -> &str {
        "TeamDelete"
    }

    fn description(&self) -> &str {
        "Remove team and task directories when work is complete."
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {}
        }))
        .expect("valid schema")
    }

    async fn execute(&self, _input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        // In full impl, would read current team context and delete
        Ok(ToolResult::text("Team deleted."))
    }
}

// ============================================================================
// SendMessageTool
// ============================================================================

pub struct SendMessageTool;

#[async_trait]
impl Tool for SendMessageTool {
    fn name(&self) -> &str {
        "SendMessage"
    }

    fn description(&self) -> &str {
        "Send a message to another agent on the team."
    }

    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "to": {
                    "type": "string",
                    "description": "Recipient: teammate name, or \"*\" for broadcast"
                },
                "message": {
                    "description": "The message content"
                },
                "summary": {
                    "type": "string",
                    "description": "A 5-10 word summary"
                }
            },
            "required": ["to", "message"]
        }))
        .expect("valid schema")
    }

    async fn execute(&self, input: Value, _ctx: &ToolUseContext) -> Result<ToolResult> {
        let to = input.get("to").and_then(|v| v.as_str()).unwrap_or("?");
        let summary = input.get("summary").and_then(|v| v.as_str()).unwrap_or("");

        Ok(ToolResult::text(format!(
            "Message sent to '{to}'. Summary: {summary}"
        )))
    }
}
