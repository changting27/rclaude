use async_trait::async_trait;
use colored::Colorize;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct ColorCommand;

/// Available agent colors for visual differentiation.
const AGENT_COLORS: &[&str] = &[
    "red", "orange", "yellow", "green", "cyan", "blue", "purple", "magenta",
];

const RESET_ALIASES: &[&str] = &["default", "reset", "none", "gray", "grey"];

#[async_trait]
impl Command for ColorCommand {
    fn name(&self) -> &str {
        "color"
    }

    fn description(&self) -> &str {
        "Set the agent color for this session"
    }

    async fn execute(&self, args: &str, _state: &mut AppState) -> Result<CommandResult> {
        let color_arg = args.trim().to_lowercase();

        if color_arg.is_empty() {
            let color_list = AGENT_COLORS.join(", ");
            return Ok(CommandResult::Ok(Some(format!(
                "Available colors: {color_list}, default"
            ))));
        }

        // Handle reset
        if RESET_ALIASES.contains(&color_arg.as_str()) {
            return Ok(CommandResult::Ok(Some(
                "Color reset to default.".to_string(),
            )));
        }

        // Validate color
        if !AGENT_COLORS.contains(&color_arg.as_str()) {
            let color_list = AGENT_COLORS.join(", ");
            return Ok(CommandResult::Ok(Some(format!(
                "{} Unknown color '{}'. Available: {color_list}, default",
                "✗".red(),
                color_arg
            ))));
        }

        // Apply color (visual feedback)
        let colored_name = match color_arg.as_str() {
            "red" => "red".red().to_string(),
            "green" => "green".green().to_string(),
            "blue" => "blue".blue().to_string(),
            "yellow" => "yellow".yellow().to_string(),
            "cyan" => "cyan".cyan().to_string(),
            "magenta" | "purple" => color_arg.magenta().to_string(),
            _ => color_arg.clone(),
        };

        Ok(CommandResult::Ok(Some(format!(
            "Color set to {colored_name}."
        ))))
    }
}
