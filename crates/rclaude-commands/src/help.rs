use async_trait::async_trait;
use colored::Colorize;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct HelpCommand;

#[async_trait]
impl Command for HelpCommand {
    fn name(&self) -> &str {
        "help"
    }

    fn description(&self) -> &str {
        "Show available commands and usage information"
    }

    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        let commands = crate::get_all_commands();

        let mut output = format!("{}\n\n", "Available commands:".bold());

        // Find max name length for alignment
        let max_len = commands.iter().map(|c| c.name().len()).max().unwrap_or(10);

        for cmd in &commands {
            output.push_str(&format!(
                "  /{:<width$}  {}\n",
                cmd.name(),
                cmd.description(),
                width = max_len
            ));
        }

        output.push_str(&format!(
            "\n{}",
            "Type your message to chat with Claude. Press Ctrl+C to exit.".dimmed()
        ));

        Ok(CommandResult::Ok(Some(output)))
    }
}
