use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct SandboxToggleCommand;
#[async_trait]
impl Command for SandboxToggleCommand {
    fn name(&self) -> &str {
        "sandbox"
    }
    fn description(&self) -> &str {
        "Toggle sandbox mode for command execution"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        // Toggle between Default and BypassPermissions as a simple sandbox toggle
        use rclaude_core::permissions::PermissionMode;
        let new_mode = if state.permission_mode == PermissionMode::Default {
            PermissionMode::BypassPermissions
        } else {
            PermissionMode::Default
        };
        state.permission_mode = new_mode;
        Ok(CommandResult::Ok(Some(format!(
            "Permission mode: {:?}",
            new_mode
        ))))
    }
}
