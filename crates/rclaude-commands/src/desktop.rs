use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct DesktopCommand;
#[async_trait]
impl Command for DesktopCommand {
    fn name(&self) -> &str {
        "desktop"
    }
    fn description(&self) -> &str {
        "Open Claude Desktop app integration"
    }
    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        Ok(CommandResult::Ok(Some("Desktop integration is available in the official Claude Code. Visit https://claude.ai/download".into())))
    }
}
