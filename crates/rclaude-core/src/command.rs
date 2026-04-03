use async_trait::async_trait;

use crate::error::Result;
use crate::state::AppState;

/// Result of executing a command.
pub enum CommandResult {
    /// Command completed successfully with optional display text.
    Ok(Option<String>),
    /// Command wants to send a message to the LLM.
    Message(String),
    /// Command wants to exit.
    Exit,
}

/// Trait for slash commands (e.g., /help, /config).
#[async_trait]
pub trait Command: Send + Sync {
    /// Command name without the leading slash (e.g., "help").
    fn name(&self) -> &str;

    /// Human-readable description.
    fn description(&self) -> &str;

    /// Whether this command is available.
    fn is_available(&self, _state: &AppState) -> bool {
        true
    }

    /// Execute the command with optional arguments.
    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult>;
}
