use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct StickersCommand;
#[async_trait]
impl Command for StickersCommand {
    fn name(&self) -> &str {
        "stickers"
    }
    fn description(&self) -> &str {
        "Get Claude stickers"
    }
    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        let _ = open::that("https://www.anthropic.com/stickers");
        Ok(CommandResult::Ok(Some(
            "Opening stickers page... 🦀".into(),
        )))
    }
}
