use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct MemoryCommand;

#[async_trait]
impl Command for MemoryCommand {
    fn name(&self) -> &str {
        "memory"
    }
    fn description(&self) -> &str {
        "Manage memory files (CLAUDE.md)"
    }

    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        let sub = args.trim();
        match sub {
            "" | "list" => {
                let memdir = state.cwd.join(".claude").join("memory");
                if !memdir.exists() {
                    return Ok(CommandResult::Ok(Some("No memory files found.".into())));
                }
                let mut files = Vec::new();
                let mut entries = tokio::fs::read_dir(&memdir).await?;
                while let Some(entry) = entries.next_entry().await? {
                    files.push(entry.file_name().to_string_lossy().to_string());
                }
                if files.is_empty() {
                    Ok(CommandResult::Ok(Some("No memory files.".into())))
                } else {
                    Ok(CommandResult::Ok(Some(format!(
                        "Memory files:\n  {}",
                        files.join("\n  ")
                    ))))
                }
            }
            _ => Ok(CommandResult::Ok(Some("Usage: /memory [list]".into()))),
        }
    }
}
