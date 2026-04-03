//! TUI runner: sets up terminal, runs event loop, cleans up.

use crossterm::{
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use std::io::{self, stdout};
use std::time::Duration;

use crate::app::{ChatRole, TuiState};
use crate::input::{poll_input, InputAction};
use crate::ui;

/// Message from the TUI to the main loop.
pub enum TuiEvent {
    /// User submitted a text message.
    UserMessage(String),
    /// User entered a slash command.
    SlashCommand(String),
    /// User wants to quit.
    Quit,
}

/// Callback type for processing user input (async).
pub type MessageHandler = Box<
    dyn Fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>>
        + Send
        + Sync,
>;

/// Initialize the terminal for TUI mode.
pub fn init_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
    terminal::enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    Terminal::new(backend)
}

/// Restore the terminal to normal mode.
pub fn restore_terminal() {
    terminal::disable_raw_mode().ok();
    stdout().execute(LeaveAlternateScreen).ok();
}

/// Run one TUI frame: render + poll input.
/// Returns a TuiEvent if the user did something, None otherwise.
pub fn tick(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut TuiState,
) -> io::Result<Option<TuiEvent>> {
    // Render
    terminal.draw(|frame| ui::render(frame, state))?;

    // Tick spinner
    state.tick_spinner();

    // Poll input (16ms ≈ 60fps)
    match poll_input(state, Duration::from_millis(16))? {
        InputAction::Submit(text) => Ok(Some(TuiEvent::UserMessage(text))),
        InputAction::Command(cmd) => Ok(Some(TuiEvent::SlashCommand(cmd))),
        InputAction::Quit => Ok(Some(TuiEvent::Quit)),
        InputAction::ScrollUp => {
            state.scroll_offset = state.scroll_offset.saturating_add(3);
            Ok(None)
        }
        InputAction::ScrollDown => {
            state.scroll_offset = state.scroll_offset.saturating_sub(3);
            Ok(None)
        }
        InputAction::None => Ok(None),
    }
}

/// Append a streamed text chunk to the latest assistant message.
pub fn append_stream_text(state: &mut TuiState, text: &str) {
    if let Some(last) = state.messages.last_mut() {
        if last.role == ChatRole::Assistant {
            last.content.push_str(text);
            return;
        }
    }
    // Create new assistant message
    state.add_message(ChatRole::Assistant, text);
}
