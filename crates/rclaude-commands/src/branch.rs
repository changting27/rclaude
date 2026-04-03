//! /branch — List or create git branches directly.

use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct BranchCommand;

#[async_trait]
impl Command for BranchCommand {
    fn name(&self) -> &str {
        "branch"
    }
    fn description(&self) -> &str {
        "List or create git branches"
    }
    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        if !state.is_git {
            return Ok(CommandResult::Ok(Some("Not in a git repository.".into())));
        }
        let name = args.trim();
        if name.is_empty() {
            // List branches
            let output = tokio::process::Command::new("git")
                .args(["branch", "-a", "--sort=-committerdate"])
                .current_dir(&state.cwd)
                .output()
                .await;
            match output {
                Ok(o) => Ok(CommandResult::Ok(Some(
                    String::from_utf8_lossy(&o.stdout).to_string(),
                ))),
                Err(e) => Ok(CommandResult::Ok(Some(format!("git branch failed: {e}")))),
            }
        } else {
            // Create and switch
            let output = tokio::process::Command::new("git")
                .args(["checkout", "-b", name])
                .current_dir(&state.cwd)
                .output()
                .await;
            match output {
                Ok(o) if o.status.success() => {
                    state.git_branch = Some(name.to_string());
                    Ok(CommandResult::Ok(Some(format!(
                        "Switched to new branch '{name}'"
                    ))))
                }
                Ok(o) => Ok(CommandResult::Ok(Some(
                    String::from_utf8_lossy(&o.stderr).to_string(),
                ))),
                Err(e) => Ok(CommandResult::Ok(Some(format!("git checkout failed: {e}")))),
            }
        }
    }
}
