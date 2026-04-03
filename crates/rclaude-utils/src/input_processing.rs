//! User input processing matching utils/processUserInput/.

use std::path::Path;

/// Processed user input result.
#[derive(Debug)]
pub struct ProcessedInput {
    pub text: String,
    pub images: Vec<String>,
    pub mentioned_files: Vec<String>,
    pub is_command: bool,
}

/// Process raw user input: expand @mentions, detect commands, extract images.
pub fn process_user_input(raw: &str, cwd: &Path) -> ProcessedInput {
    let mut text = raw.to_string();
    let mut images = Vec::new();
    let mut mentioned_files = Vec::new();
    let is_command = raw.starts_with('/');

    // Extract @file mentions
    let words: Vec<&str> = raw.split_whitespace().collect();
    for word in &words {
        if let Some(path_str) = word.strip_prefix('@') {
            let path = if Path::new(path_str).is_absolute() {
                Path::new(path_str).to_path_buf()
            } else {
                cwd.join(path_str)
            };

            if path.exists() {
                mentioned_files.push(path.to_string_lossy().to_string());

                // Check if it's an image
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if matches!(ext, "png" | "jpg" | "jpeg" | "gif" | "webp") {
                        images.push(path.to_string_lossy().to_string());
                    }
                }
            }
        }
    }

    // Clean up @mentions from text for display
    for file in &mentioned_files {
        let filename = Path::new(file)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        text = text.replace(&format!("@{filename}"), &format!("[{filename}]"));
    }

    ProcessedInput {
        text,
        images,
        mentioned_files,
        is_command,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_detection() {
        let result = process_user_input("/help", Path::new("/tmp"));
        assert!(result.is_command);
    }

    #[test]
    fn test_normal_input() {
        let result = process_user_input("explain this code", Path::new("/tmp"));
        assert!(!result.is_command);
        assert!(result.mentioned_files.is_empty());
    }
}
