use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct MobileCommand;
#[async_trait]
impl Command for MobileCommand {
    fn name(&self) -> &str {
        "mobile"
    }
    fn description(&self) -> &str {
        "Connect to mobile device"
    }
    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        Ok(CommandResult::Ok(Some("Mobile connection requires the official Claude Code app.\nSee: https://claude.ai/download".into())))
    }
}
