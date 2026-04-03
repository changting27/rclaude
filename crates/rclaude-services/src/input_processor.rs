//! User input processing matching utils/processUserInput/.
//! Handles slash commands, @mentions, and input transformation.

use std::path::Path;

/// Processed user input result.
#[derive(Debug)]
pub enum ProcessedInput {
    /// Regular message to send to the model.
    Message(String),
    /// Slash command to execute.
    SlashCommand { name: String, args: String },
    /// Empty input (ignore).
    Empty,
}

/// Process raw user input into a structured form.
pub fn process_user_input(input: &str) -> ProcessedInput {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return ProcessedInput::Empty;
    }

    // Slash commands
    if trimmed.starts_with('/') {
        let without_slash = trimmed.strip_prefix('/').unwrap_or(trimmed);
        let (name, args) = match without_slash.split_once(char::is_whitespace) {
            Some((n, a)) => (n.to_string(), a.trim().to_string()),
            None => (without_slash.to_string(), String::new()),
        };
        return ProcessedInput::SlashCommand { name, args };
    }

    ProcessedInput::Message(trimmed.to_string())
}

/// Expand @-mentions in user input to file contents.
pub fn expand_at_mentions(input: &str, cwd: &Path) -> (String, Vec<String>) {
    let (cleaned, attachments) = rclaude_core::attachments::process_at_mentions(input, cwd);
    let attachment_names: Vec<String> = attachments
        .iter()
        .map(|a| match a {
            rclaude_core::attachments::Attachment::File { filename, .. } => filename.clone(),
            rclaude_core::attachments::Attachment::Image { filename, .. } => filename.clone(),
            _ => String::new(),
        })
        .filter(|s| !s.is_empty())
        .collect();
    (cleaned, attachment_names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_message() {
        match process_user_input("hello world") {
            ProcessedInput::Message(msg) => assert_eq!(msg, "hello world"),
            _ => panic!("Expected Message"),
        }
    }

    #[test]
    fn test_process_slash_command() {
        match process_user_input("/help") {
            ProcessedInput::SlashCommand { name, args } => {
                assert_eq!(name, "help");
                assert_eq!(args, "");
            }
            _ => panic!("Expected SlashCommand"),
        }
    }

    #[test]
    fn test_process_slash_with_args() {
        match process_user_input("/model opus") {
            ProcessedInput::SlashCommand { name, args } => {
                assert_eq!(name, "model");
                assert_eq!(args, "opus");
            }
            _ => panic!("Expected SlashCommand"),
        }
    }

    #[test]
    fn test_process_empty() {
        assert!(matches!(process_user_input(""), ProcessedInput::Empty));
        assert!(matches!(process_user_input("   "), ProcessedInput::Empty));
    }
}
