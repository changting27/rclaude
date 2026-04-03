use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct DebugToolCallCommand;
#[async_trait]
impl Command for DebugToolCallCommand {
    fn name(&self) -> &str {
        "debug-tool-call"
    }
    fn description(&self) -> &str {
        "Debug the last tool call"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        // Find last tool use in messages
        for msg in state.messages.iter().rev() {
            for block in &msg.content {
                if let rclaude_core::message::ContentBlock::ToolUse { id, name, input } = block {
                    return Ok(CommandResult::Ok(Some(format!(
                        "Last tool call:\n  ID: {id}\n  Tool: {name}\n  Input: {}",
                        serde_json::to_string_pretty(input).unwrap_or_else(|_| input.to_string())
                    ))));
                }
            }
        }
        Ok(CommandResult::Ok(Some(
            "No tool calls found in conversation.".into(),
        )))
    }
}
