//! /status — Show git status directly.

use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct StatusCommand;

#[async_trait]
impl Command for StatusCommand {
    fn name(&self) -> &str {
        "status"
    }
    fn description(&self) -> &str {
        "Show git status"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        if !state.is_git {
            return Ok(CommandResult::Ok(Some("Not in a git repository.".into())));
        }
        let output = tokio::process::Command::new("git")
            .args(["status", "--short", "--branch"])
            .current_dir(&state.cwd)
            .output()
            .await;

        match output {
            Ok(o) => Ok(CommandResult::Ok(Some(
                String::from_utf8_lossy(&o.stdout).to_string(),
            ))),
            Err(e) => Ok(CommandResult::Ok(Some(format!("git status failed: {e}")))),
        }
    }
}
