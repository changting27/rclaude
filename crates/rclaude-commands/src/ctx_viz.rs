use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct CtxVizCommand;
#[async_trait]
impl Command for CtxVizCommand {
    fn name(&self) -> &str {
        "ctx-viz"
    }
    fn description(&self) -> &str {
        "Visualize context window usage"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let total_tokens =
            rclaude_core::context_window::estimate_conversation_tokens(&state.messages);
        let window = 200_000;
        let pct = (total_tokens as f64 / window as f64 * 100.0) as u32;
        let bar_len = 40;
        let filled = (pct as usize * bar_len / 100).min(bar_len);
        let bar: String = "█".repeat(filled) + &"░".repeat(bar_len - filled);
        Ok(CommandResult::Ok(Some(format!(
            "Context Window Usage:\n[{bar}] {pct}%\n{total_tokens} / {window} tokens\n{} messages",
            state.messages.len()
        ))))
    }
}
