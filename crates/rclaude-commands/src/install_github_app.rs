use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct InstallGithubAppCommand;
#[async_trait]
impl Command for InstallGithubAppCommand {
    fn name(&self) -> &str {
        "install-github-app"
    }
    fn description(&self) -> &str {
        "Install the Claude GitHub App for CI/CD integration"
    }
    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        let _ = open::that("https://github.com/apps/claude");
        Ok(CommandResult::Ok(Some(
            "Opening GitHub App installation page...\nSee: https://github.com/apps/claude".into(),
        )))
    }
}
