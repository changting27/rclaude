use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::config::Config;
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct LogoutCommand;

#[async_trait]
impl Command for LogoutCommand {
    fn name(&self) -> &str {
        "logout"
    }
    fn description(&self) -> &str {
        "Clear stored API key"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        state.config.api_key = None;
        let path = Config::config_dir().join("settings.json");
        if path.exists() {
            let json = serde_json::to_string_pretty(&state.config)?;
            tokio::fs::write(&path, json).await?;
        }
        Ok(CommandResult::Ok(Some(
            "Logged out. API key cleared.".into(),
        )))
    }
}
