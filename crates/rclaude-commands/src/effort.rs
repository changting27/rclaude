use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct EffortCommand;

#[async_trait]
impl Command for EffortCommand {
    fn name(&self) -> &str {
        "effort"
    }
    fn description(&self) -> &str {
        "Set reasoning effort level (low/medium/high)"
    }
    async fn execute(&self, args: &str, _state: &mut AppState) -> Result<CommandResult> {
        let level = args.trim();
        match level {
            "low" | "medium" | "high" | "" => {
                let display = if level.is_empty() {
                    "medium (default)"
                } else {
                    level
                };
                Ok(CommandResult::Ok(Some(format!("Effort level: {display}"))))
            }
            _ => Ok(CommandResult::Ok(Some(
                "Usage: /effort [low|medium|high]".into(),
            ))),
        }
    }
}
