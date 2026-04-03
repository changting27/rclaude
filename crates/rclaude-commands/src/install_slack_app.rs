use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct InstallSlackAppCommand;
#[async_trait]
impl Command for InstallSlackAppCommand {
    fn name(&self) -> &str {
        "install-slack-app"
    }
    fn description(&self) -> &str {
        "Install the Claude Slack App"
    }
    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        Ok(CommandResult::Ok(Some("Slack App installation is available through your Anthropic admin console.\nSee: https://docs.anthropic.com/en/docs/claude-code/slack".into())))
    }
}
