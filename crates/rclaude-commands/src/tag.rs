use async_trait::async_trait;
use colored::Colorize;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct TagCommand;

/// Get the tag file path for a session.
fn tag_file_path(cwd: &std::path::Path, session_id: &str) -> std::path::PathBuf {
    cwd.join(".claude")
        .join("sessions")
        .join(format!("{session_id}.tag"))
}

#[async_trait]
impl Command for TagCommand {
    fn name(&self) -> &str {
        "tag"
    }

    fn description(&self) -> &str {
        "Toggle a searchable tag on the current session"
    }

    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        let tag_name = args.trim();

        if tag_name.is_empty() {
            // Show current tag
            let path = tag_file_path(&state.cwd, &state.session_id);
            return match tokio::fs::read_to_string(&path).await {
                Ok(tag) if !tag.trim().is_empty() => Ok(CommandResult::Ok(Some(format!(
                    "Current tag: {}",
                    tag.trim().cyan()
                )))),
                _ => Ok(CommandResult::Ok(Some(
                    "No tag set. Usage: /tag <name>".to_string(),
                ))),
            };
        }

        let path = tag_file_path(&state.cwd, &state.session_id);

        // Toggle: if same tag exists, remove it
        if let Ok(existing) = tokio::fs::read_to_string(&path).await {
            if existing.trim() == tag_name {
                let _ = tokio::fs::remove_file(&path).await;
                return Ok(CommandResult::Ok(Some(format!(
                    "Removed tag: {}",
                    tag_name.dimmed()
                ))));
            }
        }

        // Set new tag
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let _ = tokio::fs::write(&path, tag_name).await;

        Ok(CommandResult::Ok(Some(format!(
            "Tagged session: {}",
            tag_name.cyan()
        ))))
    }
}
