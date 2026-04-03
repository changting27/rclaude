//! /doctor — Check for common issues using the diagnostics service.

use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct DoctorCommand;

#[async_trait]
impl Command for DoctorCommand {
    fn name(&self) -> &str {
        "doctor"
    }
    fn description(&self) -> &str {
        "Check for common issues and configuration problems"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let checks = rclaude_services::diagnostics::run_diagnostics(&state.cwd).await;
        let output = rclaude_services::diagnostics::format_diagnostics(&checks);
        Ok(CommandResult::Ok(Some(format!(
            "{}\n\n{}",
            colored::Colorize::bold("rclaude doctor"),
            output
        ))))
    }
}
