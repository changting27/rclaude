use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct VersionCommand;
#[async_trait]
impl Command for VersionCommand {
    fn name(&self) -> &str {
        "version"
    }
    fn description(&self) -> &str {
        "Show rclaude version"
    }
    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        Ok(CommandResult::Ok(Some(format!(
            "rclaude v{} (Rust)",
            env!("CARGO_PKG_VERSION")
        ))))
    }
}
