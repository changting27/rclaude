use async_trait::async_trait;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct SummaryCommand;

#[async_trait]
impl Command for SummaryCommand {
    fn name(&self) -> &str {
        "summary"
    }

    fn description(&self) -> &str {
        "Generate a summary of the current conversation"
    }

    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        if state.messages.is_empty() {
            return Ok(CommandResult::Ok(Some(
                "No conversation to summarize.".to_string(),
            )));
        }

        Ok(CommandResult::Message(
            "Please provide a concise summary of our conversation so far. Include:\n\
             1. The original request/goal\n\
             2. Key decisions and approaches taken\n\
             3. What was accomplished\n\
             4. Current state and any remaining work\n\
             Keep it brief but comprehensive."
                .to_string(),
        ))
    }
}
