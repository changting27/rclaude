use async_trait::async_trait;
use colored::Colorize;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

const THEMES: &[&str] = &["dark", "light", "monokai", "solarized"];

pub struct ThemeCommand;

#[async_trait]
impl Command for ThemeCommand {
    fn name(&self) -> &str {
        "theme"
    }
    fn description(&self) -> &str {
        "Change the color theme"
    }

    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        let theme = args.trim();
        if theme.is_empty() {
            let mut out = format!("Current theme: {}\n\nAvailable:", state.config.theme.cyan());
            for t in THEMES {
                out.push_str(&format!("\n  {t}"));
            }
            return Ok(CommandResult::Ok(Some(out)));
        }
        state.config.theme = theme.to_string();
        Ok(CommandResult::Ok(Some(format!(
            "Theme set to {}",
            theme.cyan()
        ))))
    }
}
