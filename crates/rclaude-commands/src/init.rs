use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct InitCommand;

#[async_trait]
impl Command for InitCommand {
    fn name(&self) -> &str {
        "init"
    }
    fn description(&self) -> &str {
        "Initialize .claude/ project configuration"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let claude_dir = state.cwd.join(".claude");
        tokio::fs::create_dir_all(&claude_dir).await?;
        let settings = claude_dir.join("settings.json");
        if !settings.exists() {
            tokio::fs::write(&settings, "{}").await?;
        }
        let claude_md = state.cwd.join("CLAUDE.md");
        if !claude_md.exists() {
            tokio::fs::write(
                &claude_md,
                "# Project Instructions\n\nAdd your project-specific instructions here.\n",
            )
            .await?;
        }
        Ok(CommandResult::Ok(Some(format!(
            "Initialized .claude/ in {}\n  Created: settings.json, CLAUDE.md",
            state.cwd.display()
        ))))
    }
}
