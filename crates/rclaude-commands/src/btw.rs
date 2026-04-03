use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct BtwCommand;

#[async_trait]
impl Command for BtwCommand {
    fn name(&self) -> &str {
        "btw"
    }
    fn description(&self) -> &str {
        "Add a side note to the conversation"
    }
    async fn execute(&self, args: &str, _state: &mut AppState) -> Result<CommandResult> {
        let note = args.trim();
        if note.is_empty() {
            Ok(CommandResult::Ok(Some("Usage: /btw <note>".into())))
        } else {
            Ok(CommandResult::Message(format!(
                "[Side note from user: {note}]\nPlease acknowledge and continue with the current task."
            )))
        }
    }
}
