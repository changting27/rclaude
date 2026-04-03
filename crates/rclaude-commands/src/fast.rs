use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct FastCommand;
#[async_trait]
impl Command for FastCommand {
    fn name(&self) -> &str {
        "fast"
    }
    fn description(&self) -> &str {
        "Toggle fast mode (use haiku for speed)"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let is_fast = state.model.contains("haiku");
        if is_fast {
            state.model = rclaude_core::model::resolve_model("sonnet");
            Ok(CommandResult::Ok(Some(format!(
                "Fast mode OFF. Model: {}",
                state.model
            ))))
        } else {
            state.model = rclaude_core::model::resolve_model("haiku");
            Ok(CommandResult::Ok(Some(format!(
                "Fast mode ON. Model: {}",
                state.model
            ))))
        }
    }
}
