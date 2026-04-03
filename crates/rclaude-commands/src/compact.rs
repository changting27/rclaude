//! /compact — Compact conversation history using API summarization.

use async_trait::async_trait;
use colored::Colorize;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct CompactCommand;

#[async_trait]
impl Command for CompactCommand {
    fn name(&self) -> &str {
        "compact"
    }

    fn description(&self) -> &str {
        "Compact conversation history (summarize old messages)"
    }

    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        if state.messages.len() <= 6 {
            return Ok(CommandResult::Ok(Some(
                "Conversation is already short, nothing to compact.".into(),
            )));
        }

        let api_key = state.config.api_key.clone().unwrap_or_default();
        if api_key.is_empty() {
            return Ok(CommandResult::Ok(Some(
                "No API key configured. Cannot compact.".into(),
            )));
        }

        let tokens_before =
            rclaude_core::context_window::estimate_conversation_tokens(&state.messages);

        match rclaude_services::compact::compact_conversation(
            &state.messages,
            &api_key,
            &state.model,
            6,
        )
        .await
        {
            Ok(result) => {
                state.messages = rclaude_services::compact::build_compacted_messages(
                    &state.messages,
                    &result.summary,
                    6,
                );
                let tokens_after =
                    rclaude_core::context_window::estimate_conversation_tokens(&state.messages);
                Ok(CommandResult::Ok(Some(format!(
                    "{}\n  Messages: {} → {}\n  Tokens: ~{} → ~{} ({} saved)",
                    "Compacted conversation".green(),
                    result.messages_before,
                    result.messages_after,
                    tokens_before,
                    tokens_after,
                    format!("~{}", tokens_before.saturating_sub(tokens_after)).cyan()
                ))))
            }
            Err(e) => {
                // Fallback to simple truncation
                let before = state.messages.len();
                state.messages = rclaude_core::context_window::compact_messages(&state.messages, 8);
                Ok(CommandResult::Ok(Some(format!(
                    "{}: {e}\nFallback: removed {} old messages.",
                    "API compact failed".yellow(),
                    before - state.messages.len()
                ))))
            }
        }
    }
}
