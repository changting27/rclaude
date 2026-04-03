use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct ShareCommand;
#[async_trait]
impl Command for ShareCommand {
    fn name(&self) -> &str {
        "share"
    }
    fn description(&self) -> &str {
        "Copy session transcript to clipboard"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let mut transcript = String::new();
        for msg in &state.messages {
            let role = match msg.role {
                rclaude_core::message::Role::User => "User",
                rclaude_core::message::Role::Assistant => "Assistant",
                rclaude_core::message::Role::System => "System",
            };
            transcript.push_str(&format!("## {role}\n{}\n\n", msg.text_content()));
        }
        // Try to copy to clipboard
        if let Ok(mut child) = std::process::Command::new("xclip")
            .arg("-selection")
            .arg("clipboard")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(stdin) = child.stdin.as_mut() {
                use std::io::Write;
                let _ = stdin.write_all(transcript.as_bytes());
            }
            let _ = child.wait();
            Ok(CommandResult::Ok(Some(
                "Transcript copied to clipboard.".into(),
            )))
        } else {
            Ok(CommandResult::Ok(Some(format!(
                "Transcript ({} messages, {} chars):\n(Install xclip to copy to clipboard)",
                state.messages.len(),
                transcript.len()
            ))))
        }
    }
}
