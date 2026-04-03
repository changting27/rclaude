use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct RewindCommand;

#[async_trait]
impl Command for RewindCommand {
    fn name(&self) -> &str {
        "rewind"
    }
    fn description(&self) -> &str {
        "Rewind conversation to a previous point"
    }
    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        let steps: usize = args.trim().parse().unwrap_or(2); // default: remove last exchange
        let to_remove = (steps * 2).min(state.messages.len());
        if to_remove == 0 {
            return Ok(CommandResult::Ok(Some("Nothing to rewind.".into())));
        }
        let new_len = state.messages.len() - to_remove;
        state.messages.truncate(new_len);
        Ok(CommandResult::Ok(Some(format!(
            "Rewound {steps} exchange(s). {} messages remaining.",
            state.messages.len()
        ))))
    }
}
