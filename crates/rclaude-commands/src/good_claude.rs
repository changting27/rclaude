//! /good_claude — Positive feedback command.

use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct GoodClaudeCommand;

#[async_trait]
impl Command for GoodClaudeCommand {
    fn name(&self) -> &str {
        "good-claude"
    }
    fn description(&self) -> &str {
        "Give positive feedback"
    }
    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        let responses = [
            "Thank you! I appreciate the feedback. 😊",
            "Thanks! That motivates me to keep doing my best.",
            "I'm glad I could help! Let me know if you need anything else.",
            "Thank you for the kind words! 🎉",
        ];
        let idx = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            % responses.len() as u128) as usize;
        Ok(CommandResult::Ok(Some(responses[idx].to_string())))
    }
}
