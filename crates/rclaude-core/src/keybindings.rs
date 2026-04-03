//! Keybinding system with default bindings and user customization.
//! Supports default bindings and user customization.

use std::collections::HashMap;

/// A keybinding action.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyAction {
    Quit,
    Cancel,
    ClearScreen,
    ClearLine,
    Submit,
    HistoryPrev,
    HistoryNext,
    HistorySearch,
    ScrollUp,
    ScrollDown,
    ScrollTop,
    ScrollBottom,
    ToggleVim,
    ToggleFast,
    NewLine,
    CopyLastResponse,
}

/// A key combination.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyCombo {
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl KeyCombo {
    pub fn ctrl(key: &str) -> Self {
        Self {
            key: key.into(),
            ctrl: true,
            alt: false,
            shift: false,
        }
    }
    pub fn shift(key: &str) -> Self {
        Self {
            key: key.into(),
            ctrl: false,
            alt: false,
            shift: true,
        }
    }
    pub fn plain(key: &str) -> Self {
        Self {
            key: key.into(),
            ctrl: false,
            alt: false,
            shift: false,
        }
    }
}

/// Keybinding registry.
pub struct KeybindingRegistry {
    bindings: HashMap<KeyCombo, KeyAction>,
}

impl KeybindingRegistry {
    /// Create with default bindings.
    pub fn default_bindings() -> Self {
        let mut bindings = HashMap::new();

        // Global
        bindings.insert(KeyCombo::ctrl("c"), KeyAction::Cancel);
        bindings.insert(KeyCombo::ctrl("d"), KeyAction::Quit);
        bindings.insert(KeyCombo::ctrl("l"), KeyAction::ClearScreen);
        bindings.insert(KeyCombo::ctrl("k"), KeyAction::ClearLine);
        bindings.insert(KeyCombo::ctrl("r"), KeyAction::HistorySearch);

        // Chat
        bindings.insert(KeyCombo::plain("Enter"), KeyAction::Submit);
        bindings.insert(KeyCombo::shift("Enter"), KeyAction::NewLine);
        bindings.insert(KeyCombo::plain("Escape"), KeyAction::Cancel);

        // History
        bindings.insert(KeyCombo::plain("Up"), KeyAction::HistoryPrev);
        bindings.insert(KeyCombo::plain("Down"), KeyAction::HistoryNext);

        // Scroll
        bindings.insert(KeyCombo::plain("PageUp"), KeyAction::ScrollUp);
        bindings.insert(KeyCombo::plain("PageDown"), KeyAction::ScrollDown);
        bindings.insert(KeyCombo::ctrl("Home"), KeyAction::ScrollTop);
        bindings.insert(KeyCombo::ctrl("End"), KeyAction::ScrollBottom);

        Self { bindings }
    }

    /// Look up the action for a key combo.
    pub fn get_action(&self, combo: &KeyCombo) -> Option<&KeyAction> {
        self.bindings.get(combo)
    }

    /// Add or override a binding.
    pub fn bind(&mut self, combo: KeyCombo, action: KeyAction) {
        self.bindings.insert(combo, action);
    }

    /// Load user keybindings from settings.
    pub fn load_user_bindings(&mut self, settings: &serde_json::Value) {
        if let Some(bindings) = settings.get("keybindings").and_then(|v| v.as_object()) {
            for (key_str, action_str) in bindings {
                if let (Some(combo), Some(action)) = (
                    parse_key_combo(key_str),
                    parse_action(action_str.as_str().unwrap_or("")),
                ) {
                    self.bindings.insert(combo, action);
                }
            }
        }
    }
}

fn parse_key_combo(s: &str) -> Option<KeyCombo> {
    let parts: Vec<&str> = s.split('+').collect();
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut key = String::new();

    for part in parts {
        match part.to_lowercase().as_str() {
            "ctrl" => ctrl = true,
            "alt" => alt = true,
            "shift" => shift = true,
            k => key = k.to_string(),
        }
    }

    if key.is_empty() {
        return None;
    }
    Some(KeyCombo {
        key,
        ctrl,
        alt,
        shift,
    })
}

fn parse_action(s: &str) -> Option<KeyAction> {
    match s.to_lowercase().as_str() {
        "quit" => Some(KeyAction::Quit),
        "cancel" => Some(KeyAction::Cancel),
        "clear_screen" | "clearscreen" => Some(KeyAction::ClearScreen),
        "clear_line" | "clearline" => Some(KeyAction::ClearLine),
        "submit" => Some(KeyAction::Submit),
        "history_prev" | "historyprev" => Some(KeyAction::HistoryPrev),
        "history_next" | "historynext" => Some(KeyAction::HistoryNext),
        "history_search" | "historysearch" => Some(KeyAction::HistorySearch),
        "scroll_up" | "scrollup" => Some(KeyAction::ScrollUp),
        "scroll_down" | "scrolldown" => Some(KeyAction::ScrollDown),
        "toggle_vim" | "togglevim" => Some(KeyAction::ToggleVim),
        "toggle_fast" | "togglefast" => Some(KeyAction::ToggleFast),
        "new_line" | "newline" => Some(KeyAction::NewLine),
        _ => None,
    }
}
