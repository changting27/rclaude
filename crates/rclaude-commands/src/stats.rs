use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct StatsCommand;

#[async_trait]
impl Command for StatsCommand {
    fn name(&self) -> &str {
        "stats"
    }
    fn description(&self) -> &str {
        "Show detailed session statistics"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let user_msgs = state
            .messages
            .iter()
            .filter(|m| m.role == rclaude_core::message::Role::User)
            .count();
        let asst_msgs = state
            .messages
            .iter()
            .filter(|m| m.role == rclaude_core::message::Role::Assistant)
            .count();
        let tokens = rclaude_core::context_window::estimate_conversation_tokens(&state.messages);
        let mut total_in = 0u64;
        let mut total_out = 0u64;
        for u in state.model_usage.values() {
            total_in += u.input_tokens;
            total_out += u.output_tokens;
        }
        Ok(CommandResult::Ok(Some(format!(
            "Session Statistics:\n  Session ID: {}\n  Messages: {} ({} user, {} assistant)\n  Est. context tokens: {}\n  API tokens: {} in / {} out\n  Cost: ${:.6}\n  Model: {}",
            &state.session_id[..8], state.messages.len(), user_msgs, asst_msgs, tokens, total_in, total_out, state.total_cost_usd, state.model
        ))))
    }
}
