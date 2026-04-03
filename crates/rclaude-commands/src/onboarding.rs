use async_trait::async_trait;
use colored::Colorize;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct OnboardingCommand;

#[async_trait]
impl Command for OnboardingCommand {
    fn name(&self) -> &str {
        "onboarding"
    }
    fn description(&self) -> &str {
        "Show getting started guide"
    }
    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        Ok(CommandResult::Ok(Some(format!(
            "{}\n\n\
             1. {} Set your API key\n   /login\n\n\
             2. {} Initialize your project\n   /init\n\n\
             3. {} Start coding\n   Just type what you want to do!\n\n\
             4. {} Useful commands\n   /help - list commands\n   /model - change model\n   /cost - check usage\n   /commit - commit changes\n\n\
             {}",
            "Welcome to rclaude!".bold().cyan(),
            "Step 1:".bold(),
            "Step 2:".bold(),
            "Step 3:".bold(),
            "Step 4:".bold(),
            "Type /help for all available commands.".dimmed(),
        ))))
    }
}
