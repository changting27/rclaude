//! Forked agent support for background task execution.
//! Manages agent execution in isolated contexts.

use std::path::PathBuf;

/// Configuration for a forked agent.
#[derive(Debug, Clone)]
pub struct ForkedAgentConfig {
    pub agent_type: String,
    pub prompt: String,
    pub model: Option<String>,
    pub max_turns: Option<usize>,
    pub cwd: PathBuf,
    pub inherit_context: bool,
}

/// Result from a forked agent execution.
#[derive(Debug)]
pub struct ForkedAgentResult {
    pub output: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}

/// Run a forked agent as a subprocess.
pub async fn run_forked_agent(config: &ForkedAgentConfig) -> Result<ForkedAgentResult, String> {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("rclaude"));
    let mut cmd = tokio::process::Command::new(&exe);
    cmd.arg("--print");

    if let Some(ref model) = config.model {
        cmd.arg(format!("--model={model}"));
    }

    cmd.arg(&config.prompt).current_dir(&config.cwd);

    let start = std::time::Instant::now();
    let output = cmd
        .output()
        .await
        .map_err(|e| format!("Fork failed: {e}"))?;
    let duration = start.elapsed();

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    let result_text = if stdout.is_empty() { stderr } else { stdout };

    Ok(ForkedAgentResult {
        output: result_text,
        exit_code: output.status.code().unwrap_or(-1),
        duration_ms: duration.as_millis() as u64,
    })
}

/// Extract the main text result from a forked agent's output.
pub fn extract_result_text(output: &str) -> String {
    // Strip any system/debug lines
    output
        .lines()
        .filter(|l| !l.starts_with("[DEBUG]") && !l.starts_with("[WARN]"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}
