use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct IdeCommand;
#[async_trait]
impl Command for IdeCommand {
    fn name(&self) -> &str {
        "ide"
    }
    fn description(&self) -> &str {
        "Configure IDE integration (VS Code, JetBrains)"
    }
    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        Ok(CommandResult::Ok(Some("IDE integration requires the official Claude Code VS Code extension or JetBrains plugin.\nSee: https://docs.anthropic.com/en/docs/claude-code/ide-integrations".into())))
    }
}
