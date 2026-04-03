use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct UpgradeCommand;
#[async_trait]
impl Command for UpgradeCommand {
    fn name(&self) -> &str {
        "upgrade"
    }
    fn description(&self) -> &str {
        "Upgrade rclaude to the latest version"
    }
    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        Ok(CommandResult::Ok(Some(
            "To upgrade rclaude:\n  cargo install --path . --force\n\nOr pull latest and rebuild:\n  git pull && cargo build --release".into()
        )))
    }
}
