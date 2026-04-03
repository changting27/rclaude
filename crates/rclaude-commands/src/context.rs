//! /context — Show context window usage with per-role breakdown.

use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::message::Role;
use rclaude_core::state::AppState;

pub struct ContextCommand;

#[async_trait]
impl Command for ContextCommand {
    fn name(&self) -> &str {
        "context"
    }
    fn description(&self) -> &str {
        "Show context window usage"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        let total = rclaude_core::context_window::estimate_conversation_tokens(&state.messages);
        let window = rclaude_core::model::context_window_for_model(&state.model);
        let pct = if window > 0 {
            (total as f64 / window as f64 * 100.0) as u32
        } else {
            0
        };

        let mut user_tokens = 0usize;
        let mut assistant_tokens = 0usize;
        let mut tool_result_tokens = 0usize;
        for msg in &state.messages {
            let t = rclaude_core::context_window::estimate_message_tokens(msg);
            match msg.role {
                Role::User => {
                    if msg.content.iter().any(|b| {
                        matches!(b, rclaude_core::message::ContentBlock::ToolResult { .. })
                    }) {
                        tool_result_tokens += t;
                    } else {
                        user_tokens += t;
                    }
                }
                Role::Assistant => assistant_tokens += t,
                Role::System => {}
            }
        }

        let warning = rclaude_core::auto_compact::calculate_warning_state(
            total,
            window,
            rclaude_core::model::max_output_for_model(&state.model) as usize,
        );
        let warning_str = match warning {
            rclaude_core::auto_compact::TokenWarningState::Normal => "normal",
            rclaude_core::auto_compact::TokenWarningState::Warning => "⚠ warning",
            rclaude_core::auto_compact::TokenWarningState::Error => "⚠ high",
            rclaude_core::auto_compact::TokenWarningState::Blocking => "🛑 blocking",
        };

        Ok(CommandResult::Ok(Some(format!(
            "Context window:\n  Model: {} ({} tokens)\n  Used: ~{} tokens ({}%)\n  Status: {}\n  Breakdown:\n    User: ~{}\n    Assistant: ~{}\n    Tool results: ~{}\n  Messages: {}",
            state.model, window, total, pct, warning_str,
            user_tokens, assistant_tokens, tool_result_tokens,
            state.messages.len()
        ))))
    }
}
