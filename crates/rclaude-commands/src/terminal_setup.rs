use async_trait::async_trait;
use colored::Colorize;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct TerminalSetupCommand;
#[async_trait]
impl Command for TerminalSetupCommand {
    fn name(&self) -> &str {
        "terminal-setup"
    }
    fn description(&self) -> &str {
        "Configure terminal for optimal rclaude experience"
    }
    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "unknown".into());
        let term = std::env::var("TERM").unwrap_or_else(|_| "unknown".into());
        let mut info = format!("{}\n", "Terminal Configuration".bold());
        info.push_str(&format!("  Shell: {shell}\n"));
        info.push_str(&format!("  TERM:  {term}\n"));
        info.push_str(&format!(
            "  UTF-8: {}\n",
            if std::env::var("LANG").unwrap_or_default().contains("UTF") {
                "yes"
            } else {
                "check LANG env"
            }
        ));
        info.push_str("\nRecommended:\n");
        info.push_str("  • Use a terminal with 256-color support\n");
        info.push_str("  • Set LANG=en_US.UTF-8 for proper Unicode rendering\n");
        info.push_str("  • Minimum 80 columns width recommended\n");
        Ok(CommandResult::Ok(Some(info)))
    }
}
