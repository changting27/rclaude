//! /diff — Show git diff directly.

use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct DiffCommand;

#[async_trait]
impl Command for DiffCommand {
    fn name(&self) -> &str {
        "diff"
    }
    fn description(&self) -> &str {
        "Show git diff of current changes"
    }
    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        if !state.is_git {
            return Ok(CommandResult::Ok(Some("Not in a git repository.".into())));
        }
        let mut cmd_args = vec!["diff"];
        let extra: Vec<&str> = args.split_whitespace().collect();
        cmd_args.extend(&extra);

        let output = tokio::process::Command::new("git")
            .args(&cmd_args)
            .current_dir(&state.cwd)
            .output()
            .await;

        match output {
            Ok(o) => {
                let text = String::from_utf8_lossy(&o.stdout);
                if text.is_empty() {
                    Ok(CommandResult::Ok(Some("No unstaged changes.".into())))
                } else {
                    Ok(CommandResult::Ok(Some(text.to_string())))
                }
            }
            Err(e) => Ok(CommandResult::Ok(Some(format!("git diff failed: {e}")))),
        }
    }
}
