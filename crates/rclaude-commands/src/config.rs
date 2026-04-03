use async_trait::async_trait;
use colored::Colorize;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct ConfigCommand;

#[async_trait]
impl Command for ConfigCommand {
    fn name(&self) -> &str {
        "config"
    }

    fn description(&self) -> &str {
        "Show or modify configuration"
    }

    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        let args = args.trim();

        if args.is_empty() {
            let output = format!(
                "{}\n  model: {}\n  max_tokens: {}\n  verbose: {}\n  theme: {}\n  config_dir: {}",
                "Current configuration:".bold(),
                state.config.model.cyan(),
                state.config.max_tokens,
                state.config.verbose,
                state.config.theme,
                rclaude_core::config::Config::config_dir().display(),
            );
            return Ok(CommandResult::Ok(Some(output)));
        }

        // Parse key=value
        if let Some((key, value)) = args.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "model" => {
                    state.config.model = value.to_string();
                    state.model = value.to_string();
                }
                "max_tokens" => {
                    if let Ok(n) = value.parse::<u32>() {
                        state.config.max_tokens = n;
                    } else {
                        return Ok(CommandResult::Ok(Some(format!(
                            "Invalid value for max_tokens: {value}"
                        ))));
                    }
                }
                "verbose" => {
                    state.config.verbose = value == "true" || value == "1";
                }
                "theme" => {
                    state.config.theme = value.to_string();
                }
                _ => {
                    return Ok(CommandResult::Ok(Some(format!(
                        "Unknown config key: {key}"
                    ))));
                }
            }
            Ok(CommandResult::Ok(Some(format!(
                "Set {} = {}",
                key.bold(),
                value.cyan()
            ))))
        } else {
            Ok(CommandResult::Ok(Some(
                "Usage: /config key=value".to_string(),
            )))
        }
    }
}
