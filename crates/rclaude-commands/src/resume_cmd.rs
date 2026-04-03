//! /resume — Resume a previous session with listing support.

use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct ResumeCommand;

#[async_trait]
impl Command for ResumeCommand {
    fn name(&self) -> &str {
        "resume"
    }
    fn description(&self) -> &str {
        "Resume a previous session"
    }
    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        let arg = args.trim();

        // /resume list — show available sessions
        if arg == "list" || arg == "ls" {
            let sessions = rclaude_services::session::list_sessions(&state.cwd).await?;
            if sessions.is_empty() {
                return Ok(CommandResult::Ok(Some("No saved sessions.".into())));
            }
            let lines: Vec<String> = sessions
                .iter()
                .take(10)
                .map(|s| {
                    format!(
                        "  {} — {} msgs, {} ({})",
                        &s.session_id[..8],
                        s.message_count,
                        s.model,
                        s.updated_at
                    )
                })
                .collect();
            return Ok(CommandResult::Ok(Some(format!(
                "Recent sessions:\n{}",
                lines.join("\n")
            ))));
        }

        // /resume <id> — resume specific session
        if !arg.is_empty() {
            let sessions = rclaude_services::session::list_sessions(&state.cwd).await?;
            if let Some(s) = sessions.iter().find(|s| s.session_id.starts_with(arg)) {
                let session =
                    rclaude_services::session::load_session(&s.session_id, &state.cwd).await?;
                if let Some(session) = session {
                    let count = session.messages.len();
                    state.messages = session.messages;
                    state.session_id = session.session_id;
                    return Ok(CommandResult::Ok(Some(format!(
                        "Resumed session {} with {count} messages.",
                        &state.session_id[..8]
                    ))));
                }
            }
            return Ok(CommandResult::Ok(Some(format!(
                "Session '{arg}' not found. Use /resume list to see available sessions."
            ))));
        }

        // /resume — resume latest
        match rclaude_services::session::load_latest_session(&state.cwd).await? {
            Some(session) => {
                let count = session.messages.len();
                state.messages = session.messages;
                state.session_id = session.session_id;
                Ok(CommandResult::Ok(Some(format!(
                    "Resumed session {} with {count} messages.",
                    &state.session_id[..8]
                ))))
            }
            None => Ok(CommandResult::Ok(Some("No previous session found.".into()))),
        }
    }
}
