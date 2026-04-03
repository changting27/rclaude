use async_trait::async_trait;
use colored::Colorize;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct KeybindingsCommand;

/// Get the keybindings config file path.
fn keybindings_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".claude")
        .join("keybindings.json")
}

/// Generate a keybindings template with default bindings.
fn generate_template() -> &'static str {
    r#"{
  // Keybinding customization for rclaude
  // Format: "action": "key" or "action": ["key1", "key2"]
  // Available actions: submit, newline, cancel, compact, help
  //
  // Default keybindings:
  //   "submit": "Enter"
  //   "newline": "Shift+Enter"
  //   "cancel": "Escape"
  //   "compact": "Ctrl+L"
  //   "help": "Ctrl+H"
}
"#
}

#[async_trait]
impl Command for KeybindingsCommand {
    fn name(&self) -> &str {
        "keybindings"
    }

    fn description(&self) -> &str {
        "Open or create your keybindings configuration file"
    }

    async fn execute(&self, _args: &str, _state: &mut AppState) -> Result<CommandResult> {
        let path = keybindings_path();

        // Create parent directory
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }

        // Create template if file doesn't exist (write-exclusive)
        let file_exists = path.exists();
        if !file_exists {
            let _ = tokio::fs::write(&path, generate_template()).await;
        }

        // Try to open in editor (matching editFileInEditor)
        let editor = std::env::var("EDITOR")
            .or_else(|_| std::env::var("VISUAL"))
            .unwrap_or_else(|_| "vi".to_string());

        let result = tokio::process::Command::new(&editor)
            .arg(&path)
            .status()
            .await;

        match result {
            Ok(status) if status.success() => {
                let action = if file_exists { "Opened" } else { "Created" };
                Ok(CommandResult::Ok(Some(format!(
                    "{} {} {}",
                    "✓".green(),
                    action,
                    path.display()
                ))))
            }
            _ => Ok(CommandResult::Ok(Some(format!(
                "{} {} Could not open editor. Edit manually: {}",
                if file_exists { "Opened" } else { "Created" },
                path.display(),
                format!("$EDITOR {}", path.display()).dimmed()
            )))),
        }
    }
}
