use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct VimCommand;
#[async_trait]
impl Command for VimCommand {
    fn name(&self) -> &str {
        "vim"
    }
    fn description(&self) -> &str {
        "Toggle between Vim and Normal editing modes"
    }
    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        // Toggle vim mode in config
        let config_dir = rclaude_core::config::Config::config_dir();
        let config_path = config_dir.join("settings.json");
        let mut settings: serde_json::Value = std::fs::read_to_string(&config_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(serde_json::json!({}));
        let current = settings
            .get("editorMode")
            .and_then(|v| v.as_str())
            .unwrap_or("normal");
        let new_mode = if current == "vim" { "normal" } else { "vim" };
        settings["editorMode"] = serde_json::Value::String(new_mode.to_string());
        let _ = std::fs::create_dir_all(&config_dir);
        let _ = std::fs::write(
            &config_path,
            serde_json::to_string_pretty(&settings).unwrap_or_default(),
        );
        let msg = if new_mode == "vim" {
            "Editor mode set to vim. Use Escape to toggle between INSERT and NORMAL modes."
        } else {
            "Editor mode set to normal. Using standard keyboard bindings."
        };
        Ok(CommandResult::Ok(Some(msg.to_string())))
    }
}
