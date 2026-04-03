use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct AddDirCommand;

#[async_trait]
impl Command for AddDirCommand {
    fn name(&self) -> &str {
        "add-dir"
    }
    fn description(&self) -> &str {
        "Add an additional working directory for tool access"
    }
    async fn execute(&self, args: &str, _state: &mut AppState) -> Result<CommandResult> {
        let dir = args.trim();
        if dir.is_empty() {
            let current = std::env::var("CLAUDE_ADD_DIRS").unwrap_or_default();
            if current.is_empty() {
                return Ok(CommandResult::Ok(Some(
                    "No additional directories. Usage: /add-dir <path>".into(),
                )));
            }
            return Ok(CommandResult::Ok(Some(format!(
                "Additional directories:\n{}",
                current
                    .split(':')
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("  {s}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ))));
        }
        let path = std::path::Path::new(dir);
        if !path.exists() {
            return Ok(CommandResult::Ok(Some(format!(
                "Directory not found: {dir}"
            ))));
        }
        let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let current = std::env::var("CLAUDE_ADD_DIRS").unwrap_or_default();
        std::env::set_var("CLAUDE_ADD_DIRS", format!("{current}:{}", abs.display()));
        Ok(CommandResult::Ok(Some(format!(
            "Added directory: {}",
            abs.display()
        ))))
    }
}
