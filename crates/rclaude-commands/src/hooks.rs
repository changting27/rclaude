use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct HooksCommand;

#[async_trait]
impl Command for HooksCommand {
    fn name(&self) -> &str {
        "hooks"
    }
    fn description(&self) -> &str {
        "Show configured hooks"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let settings_path = state.cwd.join(".claude").join("settings.json");
        if settings_path.exists() {
            let content = tokio::fs::read_to_string(&settings_path).await?;
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(hooks) = val.get("hooks") {
                    return Ok(CommandResult::Ok(Some(
                        serde_json::to_string_pretty(hooks).unwrap_or_else(|_| "{}".into()),
                    )));
                }
            }
        }
        Ok(CommandResult::Ok(Some(
            "No hooks configured. Add to .claude/settings.json".into(),
        )))
    }
}
