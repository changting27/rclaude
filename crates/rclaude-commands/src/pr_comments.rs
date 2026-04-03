use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct PrCommentsCommand;

#[async_trait]
impl Command for PrCommentsCommand {
    fn name(&self) -> &str {
        "pr-comments"
    }
    fn description(&self) -> &str {
        "View comments on a GitHub PR"
    }
    async fn execute(&self, args: &str, _state: &mut AppState) -> Result<CommandResult> {
        let pr = args.trim();
        if pr.is_empty() {
            return Ok(CommandResult::Ok(Some(
                "Usage: /pr_comments <PR number or URL>".into(),
            )));
        }
        Ok(CommandResult::Message(format!(
            "Run `gh pr view {pr} --comments` and show me the output."
        )))
    }
}
