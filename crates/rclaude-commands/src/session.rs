use async_trait::async_trait;
use colored::Colorize;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct SessionCommand;

#[async_trait]
impl Command for SessionCommand {
    fn name(&self) -> &str {
        "session"
    }

    fn description(&self) -> &str {
        "Show session information"
    }

    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let output = format!(
            "{}\n  ID: {}\n  CWD: {}\n  Model: {}\n  Messages: {}\n  Git: {}{}",
            "Session info:".bold(),
            &state.session_id[..8],
            state.cwd.display(),
            state.model.cyan(),
            state.messages.len(),
            if state.is_git { "yes" } else { "no" },
            state
                .git_branch
                .as_ref()
                .map(|b| format!(" ({})", b))
                .unwrap_or_default(),
        );

        Ok(CommandResult::Ok(Some(output)))
    }
}
