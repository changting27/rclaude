use rclaude_core::error::Result;
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;

/// Result of executing a shell command.
#[derive(Debug)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Execute a shell command and capture output.
pub async fn exec(command: &str, cwd: &Path, timeout: Option<Duration>) -> Result<ExecResult> {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command).current_dir(cwd);

    let output = if let Some(timeout) = timeout {
        tokio::time::timeout(timeout, cmd.output())
            .await
            .map_err(|_| {
                rclaude_core::error::RclaudeError::Timeout(format!(
                    "Command timed out after {}ms",
                    timeout.as_millis()
                ))
            })??
    } else {
        cmd.output().await?
    };

    Ok(ExecResult {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}
