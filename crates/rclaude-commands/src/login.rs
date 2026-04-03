use async_trait::async_trait;
use colored::Colorize;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::config::Config;
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct LoginCommand;

#[async_trait]
impl Command for LoginCommand {
    fn name(&self) -> &str {
        "login"
    }

    fn description(&self) -> &str {
        "Set your Anthropic API key"
    }

    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        let key = args.trim();

        if key.is_empty() {
            // Interactive mode: prompt for key
            eprint!("Enter your Anthropic API key: ");
            use std::io::Write;
            std::io::stderr().flush().ok();

            let mut input = String::new();
            std::io::stdin()
                .read_line(&mut input)
                .map_err(|e| rclaude_core::error::RclaudeError::Other(e.to_string()))?;

            let key = input.trim().to_string();
            if key.is_empty() {
                return Ok(CommandResult::Ok(Some("No key provided.".to_string())));
            }

            if !key.starts_with("sk-") {
                return Ok(CommandResult::Ok(Some(
                    "Warning: API key doesn't start with 'sk-'. Are you sure this is correct?"
                        .to_string(),
                )));
            }

            state.config.api_key = Some(key);
            save_api_key(&state.config).await;
            Ok(CommandResult::Ok(Some(
                "API key saved.".green().to_string(),
            )))
        } else {
            state.config.api_key = Some(key.to_string());
            save_api_key(&state.config).await;
            Ok(CommandResult::Ok(Some(
                "API key saved.".green().to_string(),
            )))
        }
    }
}

async fn save_api_key(config: &Config) {
    let dir = Config::config_dir();
    tokio::fs::create_dir_all(&dir).await.ok();
    let path = dir.join("settings.json");
    if let Ok(json) = serde_json::to_string_pretty(config) {
        tokio::fs::write(&path, json).await.ok();
    }
}
