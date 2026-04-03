use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct IssueCommand;

#[async_trait]
impl Command for IssueCommand {
    fn name(&self) -> &str {
        "issue"
    }
    fn description(&self) -> &str {
        "Create or view a GitHub issue"
    }
    async fn execute(&self, args: &str, _state: &mut AppState) -> Result<CommandResult> {
        let args = args.trim();
        if args.is_empty() {
            return Ok(CommandResult::Message(
                "Run `gh issue list` and show me the open issues.".into(),
            ));
        }
        if let Ok(_num) = args.parse::<u64>() {
            Ok(CommandResult::Message(format!(
                "Run `gh issue view {args}` and show me the details."
            )))
        } else {
            Ok(CommandResult::Message(format!(
                "Create a GitHub issue with title: {args}. Run `gh issue create --title \"{args}\"` and let me fill in the body."
            )))
        }
    }
}
