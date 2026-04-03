use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct CopyCommand;

#[async_trait]
impl Command for CopyCommand {
    fn name(&self) -> &str {
        "copy"
    }
    fn description(&self) -> &str {
        "Copy last assistant response to clipboard"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let last = state
            .messages
            .iter()
            .rev()
            .find(|m| m.role == rclaude_core::message::Role::Assistant);
        match last {
            Some(msg) => {
                let text = msg.text_content();
                // Try to pipe to clipboard
                let result = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg("command -v pbcopy >/dev/null && pbcopy || command -v xclip >/dev/null && xclip -selection clipboard || command -v xsel >/dev/null && xsel --clipboard --input")
                    .stdin(std::process::Stdio::piped())
                    .spawn();
                match result {
                    Ok(mut child) => {
                        if let Some(mut stdin) = child.stdin.take() {
                            use tokio::io::AsyncWriteExt;
                            stdin.write_all(text.as_bytes()).await.ok();
                        }
                        child.wait().await.ok();
                        Ok(CommandResult::Ok(Some("Copied to clipboard.".into())))
                    }
                    Err(_) => Ok(CommandResult::Ok(Some(
                        "No clipboard tool found (pbcopy/xclip/xsel).".into(),
                    ))),
                }
            }
            None => Ok(CommandResult::Ok(Some(
                "No assistant message to copy.".into(),
            ))),
        }
    }
}
