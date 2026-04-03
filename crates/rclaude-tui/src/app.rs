//! TUI application state and event handling.

/// A single message displayed in the chat area.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    User,
    Assistant,
    System,
    Tool,
}

/// TUI application state (separate from core AppState).
pub struct TuiState {
    /// Chat messages to display.
    pub messages: Vec<ChatMessage>,
    /// Current input text.
    pub input: String,
    /// Cursor position in input.
    pub cursor_pos: usize,
    /// Scroll offset for the chat area.
    pub scroll_offset: u16,
    /// Whether we're waiting for an API response.
    pub is_loading: bool,
    /// Status bar text (model, cost, branch).
    pub status_model: String,
    pub status_branch: Option<String>,
    pub status_cost: f64,
    pub status_tokens: u64,
    /// Whether the app should quit.
    pub should_quit: bool,
    /// Spinner frame for loading animation.
    pub spinner_frame: usize,
    /// Q11: Current loading status message (e.g., "Running Bash...", "Reading file...").
    pub loading_status: Option<String>,
    /// Input history.
    history: Vec<String>,
    /// Current position in history (-1 = not browsing).
    history_pos: Option<usize>,
}

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

impl TuiState {
    pub fn new(model: &str, branch: Option<String>) -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            cursor_pos: 0,
            scroll_offset: 0,
            is_loading: false,
            status_model: model.to_string(),
            status_branch: branch,
            status_cost: 0.0,
            status_tokens: 0,
            should_quit: false,
            spinner_frame: 0,
            loading_status: None,
            history: Vec::new(),
            history_pos: None,
        }
    }

    pub fn add_message(&mut self, role: ChatRole, content: impl Into<String>) {
        self.messages.push(ChatMessage {
            role,
            content: content.into(),
        });
    }

    pub fn spinner(&self) -> &str {
        SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()]
    }

    pub fn tick_spinner(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
    }

    /// Push a new entry to input history.
    pub fn push_history(&mut self, text: String) {
        // Avoid consecutive duplicates
        if self.history.last() != Some(&text) {
            self.history.push(text);
        }
        self.history_pos = None;
    }

    /// Navigate to previous history entry.
    pub fn history_prev(&mut self) -> Option<String> {
        if self.history.is_empty() {
            return None;
        }
        let pos = match self.history_pos {
            Some(p) if p > 0 => p - 1,
            Some(_) => return None,
            None => self.history.len() - 1,
        };
        self.history_pos = Some(pos);
        Some(self.history[pos].clone())
    }

    /// Navigate to next history entry.
    pub fn history_next(&mut self) -> Option<String> {
        let pos = self.history_pos?;
        if pos + 1 < self.history.len() {
            self.history_pos = Some(pos + 1);
            Some(self.history[pos + 1].clone())
        } else {
            self.history_pos = None;
            None
        }
    }
}
