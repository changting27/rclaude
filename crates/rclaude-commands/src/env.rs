use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct EnvCommand;

#[async_trait]
impl Command for EnvCommand {
    fn name(&self) -> &str {
        "env"
    }
    fn description(&self) -> &str {
        "Show environment information"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let info = format!(
            "CWD: {}\nOS: {}\nArch: {}\nModel: {}\nGit: {}\nShell: {}",
            state.cwd.display(),
            std::env::consts::OS,
            std::env::consts::ARCH,
            state.model,
            if state.is_git { "yes" } else { "no" },
            std::env::var("SHELL").unwrap_or_else(|_| "unknown".into()),
        );
        Ok(CommandResult::Ok(Some(info)))
    }
}
