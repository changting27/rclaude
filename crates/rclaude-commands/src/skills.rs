use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct SkillsCommand;

#[async_trait]
impl Command for SkillsCommand {
    fn name(&self) -> &str {
        "skills"
    }
    fn description(&self) -> &str {
        "List available skills"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let skills_dir = state.cwd.join(".claude").join("skills");
        if !skills_dir.exists() {
            return Ok(CommandResult::Ok(Some(
                "No skills found. Create in .claude/skills/".into(),
            )));
        }
        let mut names = Vec::new();
        let mut entries = tokio::fs::read_dir(&skills_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                names.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        if names.is_empty() {
            Ok(CommandResult::Ok(Some("No skills found.".into())))
        } else {
            Ok(CommandResult::Ok(Some(format!(
                "Skills:\n  {}",
                names.join("\n  ")
            ))))
        }
    }
}
