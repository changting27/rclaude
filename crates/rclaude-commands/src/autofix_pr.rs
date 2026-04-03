use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct AutofixPrCommand;

#[async_trait]
impl Command for AutofixPrCommand {
    fn name(&self) -> &str {
        "autofix-pr"
    }
    fn description(&self) -> &str {
        "Automatically fix issues in a pull request"
    }
    async fn execute(&self, args: &str, _state: &mut AppState) -> Result<CommandResult> {
        let pr = args.trim();
        if pr.is_empty() {
            return Ok(CommandResult::Ok(Some(
                "Usage: /autofix-pr <PR number>".into(),
            )));
        }
        Ok(CommandResult::Message(format!(
            "Review PR #{pr}:\n1. Run `gh pr diff {pr}` to see changes\n\
             2. Run `gh pr checks {pr}` for CI status\n\
             3. Fix any failing checks or review comments\n\
             4. Push fixes"
        )))
    }
}
