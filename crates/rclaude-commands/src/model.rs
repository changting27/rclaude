use async_trait::async_trait;
use colored::Colorize;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct ModelCommand;

#[async_trait]
impl Command for ModelCommand {
    fn name(&self) -> &str {
        "model"
    }

    fn description(&self) -> &str {
        "Show or change the current model"
    }

    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        let args = args.trim();

        if args.is_empty() {
            let resolved = rclaude_core::model::resolve_model(&state.model);
            let mut output = format!(
                "{} {} ({})\n\n",
                "Current model:".bold(),
                state.model.cyan(),
                resolved.dimmed()
            );

            output.push_str("Available models:\n");
            // Show aliases with their resolved names
            for (alias, desc) in [
                ("sonnet", "Sonnet — balanced speed and capability"),
                ("opus", "Opus — most capable for complex work"),
                ("haiku", "Haiku — fastest for quick answers"),
            ] {
                let resolved_name = rclaude_core::model::resolve_model(alias);
                let is_current = state.model.to_lowercase().contains(alias);
                let marker = if is_current { " ✔" } else { "" };
                output.push_str(&format!(
                    "  {} {} {}{}\n",
                    if is_current {
                        "❯".green().to_string()
                    } else {
                        " ".to_string()
                    },
                    alias.cyan(),
                    resolved_name.dimmed(),
                    marker.green()
                ));
                let _ = desc; // description available for verbose mode
            }

            output.push_str(&format!(
                "\nUsage: {} or {}",
                "/model sonnet".cyan(),
                "/model <full-model-name>".cyan()
            ));
            return Ok(CommandResult::Ok(Some(output)));
        }

        // Set the model
        let resolved = rclaude_core::model::resolve_model(args);
        state.model = resolved.clone();
        state.config.model = args.to_string();

        Ok(CommandResult::Ok(Some(format!(
            "Model changed to {} ({})",
            args.cyan(),
            resolved.dimmed()
        ))))
    }
}
