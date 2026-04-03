//! Input handling for the TUI.

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

use crate::app::TuiState;

/// Action resulting from input processing.
pub enum InputAction {
    None,
    Submit(String),
    Quit,
    Command(String),
    ScrollUp,
    ScrollDown,
}

/// Poll for input events and update state. Non-blocking with timeout.
pub fn poll_input(state: &mut TuiState, timeout: Duration) -> std::io::Result<InputAction> {
    if !event::poll(timeout)? {
        return Ok(InputAction::None);
    }

    match event::read()? {
        Event::Key(key) => Ok(handle_key(state, key)),
        _ => Ok(InputAction::None),
    }
}

fn handle_key(state: &mut TuiState, key: KeyEvent) -> InputAction {
    // Ctrl+C / Ctrl+D → quit
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('c') | KeyCode::Char('d') => return InputAction::Quit,
            KeyCode::Char('l') => {
                // Clear screen — reset messages
                state.messages.clear();
                state.scroll_offset = 0;
                return InputAction::None;
            }
            KeyCode::Char('k') => {
                // Clear input line
                state.input.clear();
                state.cursor_pos = 0;
                return InputAction::None;
            }
            _ => {}
        }
    }

    match key.code {
        // Q11: Shift+Enter for multi-line input
        KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
            state.input.insert(state.cursor_pos, '\n');
            state.cursor_pos += 1;
            InputAction::None
        }
        KeyCode::Enter => {
            let text = state.input.trim().to_string();
            if text.is_empty() {
                return InputAction::None;
            }
            // Save to history
            state.push_history(text.clone());
            state.input.clear();
            state.cursor_pos = 0;

            if text.starts_with('/') {
                InputAction::Command(text)
            } else {
                InputAction::Submit(text)
            }
        }
        KeyCode::Char(c) => {
            state.input.insert(state.cursor_pos, c);
            state.cursor_pos += c.len_utf8();
            InputAction::None
        }
        KeyCode::Backspace => {
            if state.cursor_pos > 0 {
                let mut new_pos = state.cursor_pos - 1;
                while new_pos > 0 && !state.input.is_char_boundary(new_pos) {
                    new_pos -= 1;
                }
                state.input.drain(new_pos..state.cursor_pos);
                state.cursor_pos = new_pos;
            }
            InputAction::None
        }
        KeyCode::Delete => {
            if state.cursor_pos < state.input.len() {
                state.input.remove(state.cursor_pos);
            }
            InputAction::None
        }
        KeyCode::Left => {
            if state.cursor_pos > 0 {
                let mut new_pos = state.cursor_pos - 1;
                while new_pos > 0 && !state.input.is_char_boundary(new_pos) {
                    new_pos -= 1;
                }
                state.cursor_pos = new_pos;
            }
            InputAction::None
        }
        KeyCode::Right => {
            if state.cursor_pos < state.input.len() {
                let mut new_pos = state.cursor_pos + 1;
                while new_pos < state.input.len() && !state.input.is_char_boundary(new_pos) {
                    new_pos += 1;
                }
                state.cursor_pos = new_pos;
            }
            InputAction::None
        }
        // Arrow up/down: input history navigation
        KeyCode::Up => {
            if let Some(prev) = state.history_prev() {
                state.input = prev;
                state.cursor_pos = state.input.len();
            }
            InputAction::None
        }
        KeyCode::Down => {
            if let Some(next) = state.history_next() {
                state.input = next;
                state.cursor_pos = state.input.len();
            } else {
                state.input.clear();
                state.cursor_pos = 0;
            }
            InputAction::None
        }
        KeyCode::Home => {
            state.cursor_pos = 0;
            InputAction::None
        }
        KeyCode::End => {
            state.cursor_pos = state.input.len();
            InputAction::None
        }
        KeyCode::PageUp => InputAction::ScrollUp,
        KeyCode::PageDown => InputAction::ScrollDown,
        _ => InputAction::None,
    }
}
