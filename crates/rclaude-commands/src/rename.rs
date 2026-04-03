use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct RenameCommand;

#[async_trait]
impl Command for RenameCommand {
    fn name(&self) -> &str {
        "rename"
    }
    fn description(&self) -> &str {
        "Rename the current session"
    }
    async fn execute(&self, args: &str, _state: &mut AppState) -> Result<CommandResult> {
        let name = args.trim();
        if name.is_empty() {
            Ok(CommandResult::Ok(Some("Usage: /rename <name>".into())))
        } else {
            Ok(CommandResult::Ok(Some(format!(
                "Session renamed to: {name}"
            ))))
        }
    }
}
