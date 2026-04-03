//! Shell provider matching utils/shell/.
//! Manages shell execution environments.

use std::path::PathBuf;

/// Shell provider configuration.
#[derive(Debug, Clone)]
pub struct ShellProvider {
    pub shell_path: PathBuf,
    pub shell_type: ShellType,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    Sh,
}

impl ShellProvider {
    /// Detect the default shell.
    pub fn detect() -> Self {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        let shell_type = if shell.contains("zsh") {
            ShellType::Zsh
        } else if shell.contains("fish") {
            ShellType::Fish
        } else if shell.contains("bash") {
            ShellType::Bash
        } else {
            ShellType::Sh
        };

        Self {
            shell_path: PathBuf::from(&shell),
            shell_type,
            args: vec!["-c".into()],
        }
    }

    /// Build command args for executing a command string.
    pub fn build_args(&self, command: &str) -> Vec<String> {
        let mut args = self.args.clone();
        args.push(command.to_string());
        args
    }
}

/// Get max output length for tool results.
pub fn get_max_output_length() -> usize {
    std::env::var("CLAUDE_MAX_OUTPUT_LENGTH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30_000)
}

/// Truncate output to max length with indicator.
pub fn truncate_output(output: &str, max_len: usize) -> String {
    if output.len() <= max_len {
        return output.to_string();
    }
    let truncated = &output[..max_len];
    format!(
        "{truncated}\n... (truncated, {total} chars total)",
        total = output.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        let short = "hello";
        assert_eq!(truncate_output(short, 100), "hello");
        let long = "a".repeat(100);
        let result = truncate_output(&long, 50);
        assert!(result.contains("truncated"));
    }

    #[test]
    fn test_max_output() {
        assert!(get_max_output_length() > 0);
    }
}
