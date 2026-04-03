use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct ReleaseNotesCommand;

#[async_trait]
impl Command for ReleaseNotesCommand {
    fn name(&self) -> &str {
        "release-notes"
    }
    fn description(&self) -> &str {
        "Generate release notes from recent commits"
    }
    async fn execute(&self, args: &str, _state: &mut AppState) -> Result<CommandResult> {
        let range = if args.trim().is_empty() {
            "HEAD~10..HEAD"
        } else {
            args.trim()
        };
        Ok(CommandResult::Message(format!(
            "Run `git log {range} --oneline --no-merges` to see recent commits, \
             then generate release notes in markdown format with categories: \
             Features, Bug Fixes, Improvements, Breaking Changes."
        )))
    }
}
