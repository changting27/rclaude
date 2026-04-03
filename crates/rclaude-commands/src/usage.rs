use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct UsageCommand;

#[async_trait]
impl Command for UsageCommand {
    fn name(&self) -> &str {
        "usage"
    }
    fn description(&self) -> &str {
        "Show API usage statistics"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let mut total_in = 0u64;
        let mut total_out = 0u64;
        for usage in state.model_usage.values() {
            total_in += usage.input_tokens;
            total_out += usage.output_tokens;
        }
        Ok(CommandResult::Ok(Some(format!(
            "API Usage:\n  Input: {} tokens\n  Output: {} tokens\n  Total: {} tokens\n  Cost: ${:.6}",
            total_in, total_out, total_in + total_out, state.total_cost_usd
        ))))
    }
}
