use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct AdvisorCommand;
#[async_trait]
impl Command for AdvisorCommand {
    fn name(&self) -> &str {
        "advisor"
    }
    fn description(&self) -> &str {
        "Get suggestions for what to work on next"
    }
    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        Ok(CommandResult::Message(
            "Based on the current project state, suggest what I should work on next. \
             Consider:\n\
             1. Any failing tests or build errors\n\
             2. Open TODOs in the codebase\n\
             3. Code quality improvements\n\
             4. Missing documentation\n\
             5. Performance optimizations\n\
             Prioritize by impact and provide actionable next steps."
                .into(),
        ))
    }
}
