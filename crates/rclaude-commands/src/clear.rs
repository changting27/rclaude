use async_trait::async_trait;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct ClearCommand;

#[async_trait]
impl Command for ClearCommand {
    fn name(&self) -> &str {
        "clear"
    }

    fn description(&self) -> &str {
        "Clear conversation history"
    }

    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let count = state.messages.len();
        state.messages.clear();
        Ok(CommandResult::Ok(Some(format!(
            "Cleared {count} messages from conversation."
        ))))
    }
}
