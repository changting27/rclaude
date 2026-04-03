use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct PlanCommand;

#[async_trait]
impl Command for PlanCommand {
    fn name(&self) -> &str {
        "plan"
    }
    fn description(&self) -> &str {
        "Enter or exit plan mode"
    }
    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        match args.trim() {
            "off" | "exit" => {
                state.permission_mode = rclaude_core::permissions::PermissionMode::Default;
                Ok(CommandResult::Ok(Some("Plan mode disabled.".into())))
            }
            _ => {
                state.permission_mode = rclaude_core::permissions::PermissionMode::Plan;
                Ok(CommandResult::Ok(Some(
                    "Plan mode enabled (read-only). Use /plan off to exit.".into(),
                )))
            }
        }
    }
}
