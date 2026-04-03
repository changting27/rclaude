use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct ExportCommand;

#[async_trait]
impl Command for ExportCommand {
    fn name(&self) -> &str {
        "export"
    }
    fn description(&self) -> &str {
        "Export conversation as JSON or markdown"
    }

    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        let format = args.trim();
        let format = if format.is_empty() { "json" } else { format };

        match format {
            "json" => {
                let json = serde_json::to_string_pretty(&state.messages)?;
                let path = state
                    .cwd
                    .join(format!("conversation-{}.json", &state.session_id[..8]));
                tokio::fs::write(&path, &json).await?;
                Ok(CommandResult::Ok(Some(format!(
                    "Exported to {}",
                    path.display()
                ))))
            }
            "md" | "markdown" => {
                let mut md = String::from("# Conversation\n\n");
                for msg in &state.messages {
                    let role = match msg.role {
                        rclaude_core::message::Role::User => "**User**",
                        rclaude_core::message::Role::Assistant => "**Assistant**",
                        rclaude_core::message::Role::System => "**System**",
                    };
                    md.push_str(&format!("## {role}\n\n{}\n\n", msg.text_content()));
                }
                let path = state
                    .cwd
                    .join(format!("conversation-{}.md", &state.session_id[..8]));
                tokio::fs::write(&path, &md).await?;
                Ok(CommandResult::Ok(Some(format!(
                    "Exported to {}",
                    path.display()
                ))))
            }
            _ => Ok(CommandResult::Ok(Some("Usage: /export [json|md]".into()))),
        }
    }
}
