use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct ExitCommand;
#[async_trait]
impl Command for ExitCommand {
    fn name(&self) -> &str {
        "exit"
    }
    fn description(&self) -> &str {
        "Exit rclaude"
    }
    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        Ok(CommandResult::Exit)
    }
}
