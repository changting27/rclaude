use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct AgentsCommand;
#[async_trait]
impl Command for AgentsCommand {
    fn name(&self) -> &str {
        "agents"
    }
    fn description(&self) -> &str {
        "List available agent definitions"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let agents_dir = state.cwd.join(".claude").join("agents");
        if !agents_dir.exists() {
            return Ok(CommandResult::Ok(Some(
                "No agent definitions. Create in .claude/agents/".into(),
            )));
        }
        let mut names = Vec::new();
        let mut entries = tokio::fs::read_dir(&agents_dir).await?;
        while let Some(e) = entries.next_entry().await? {
            names.push(e.file_name().to_string_lossy().to_string());
        }
        Ok(CommandResult::Ok(Some(format!(
            "Agents:\n  {}",
            names.join("\n  ")
        ))))
    }
}
