use async_trait::async_trait;
use colored::Colorize;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct PermissionsCommand;

#[async_trait]
impl Command for PermissionsCommand {
    fn name(&self) -> &str {
        "permissions"
    }
    fn description(&self) -> &str {
        "Show or change permission mode"
    }
    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        let arg = args.trim();
        if arg.is_empty() {
            return Ok(CommandResult::Ok(Some(format!(
                "Permission mode: {:?}\n\nAvailable: default, auto, plan, bypass",
                state.permission_mode
            ))));
        }
        match arg {
            "default" => state.permission_mode = rclaude_core::permissions::PermissionMode::Default,
            "auto" => state.permission_mode = rclaude_core::permissions::PermissionMode::Auto,
            "plan" => state.permission_mode = rclaude_core::permissions::PermissionMode::Plan,
            "bypass" => {
                state.permission_mode = rclaude_core::permissions::PermissionMode::BypassPermissions
            }
            _ => return Ok(CommandResult::Ok(Some(format!("Unknown mode: {arg}")))),
        }
        Ok(CommandResult::Ok(Some(format!(
            "Permission mode set to {}",
            arg.cyan()
        ))))
    }
}
