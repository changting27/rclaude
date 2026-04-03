use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct OutputStyleCommand;

#[async_trait]
impl Command for OutputStyleCommand {
    fn name(&self) -> &str {
        "output-style"
    }
    fn description(&self) -> &str {
        "Set output style (default, explanatory, learning)"
    }
    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        let style = args.trim().to_lowercase();
        if style.is_empty() {
            let current = state.config.output_style.as_deref().unwrap_or("default");
            let mut output = format!("Current style: {current}\n\nAvailable styles:\n");
            output.push_str("  default      — Standard concise output\n");
            output.push_str("  explanatory  — Educational insights alongside actions\n");
            output.push_str("  learning     — Hands-on practice with guided exercises\n");
            return Ok(CommandResult::Ok(Some(output)));
        }
        match style.as_str() {
            "default" => {
                state.config.output_style = None;
                Ok(CommandResult::Ok(Some("Output style: default".into())))
            }
            "explanatory" | "learning" => {
                state.config.output_style = Some(style.clone());
                Ok(CommandResult::Ok(Some(format!("Output style: {style}"))))
            }
            _ => Ok(CommandResult::Ok(Some(
                "Unknown style. Use: default, explanatory, learning".into(),
            ))),
        }
    }
}
