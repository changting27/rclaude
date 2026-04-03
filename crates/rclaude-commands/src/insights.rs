use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct InsightsCommand;
#[async_trait]
impl Command for InsightsCommand {
    fn name(&self) -> &str {
        "insights"
    }
    fn description(&self) -> &str {
        "Show insights about the conversation"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let total = state.messages.len();
        let tools_used = state
            .messages
            .iter()
            .flat_map(|m| m.content.iter())
            .filter(|b| matches!(b, rclaude_core::message::ContentBlock::ToolUse { .. }))
            .count();
        Ok(CommandResult::Ok(Some(format!(
            "Insights:\n  Messages: {total}\n  Tool calls: {tools_used}\n  Models used: {:?}",
            state.model_usage.keys().collect::<Vec<_>>()
        ))))
    }
}
