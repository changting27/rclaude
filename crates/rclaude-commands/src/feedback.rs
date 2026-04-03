use async_trait::async_trait;
use colored::Colorize;

use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct FeedbackCommand;

/// Collect environment info for the feedback report.
fn collect_env_info(state: &AppState) -> String {
    let mut info = String::new();
    info.push_str(&format!(
        "version: rclaude v{}\n",
        env!("CARGO_PKG_VERSION")
    ));
    info.push_str(&format!("platform: {}\n", std::env::consts::OS));
    info.push_str(&format!("arch: {}\n", std::env::consts::ARCH));
    info.push_str(&format!("git_repo: {}\n", state.is_git));
    if let Some(ref branch) = state.git_branch {
        info.push_str(&format!("git_branch: {branch}\n"));
    }
    info.push_str(&format!("model: {}\n", state.model));
    info.push_str(&format!("message_count: {}\n", state.messages.len()));
    info.push_str(&format!("session_id: {}\n", state.session_id));
    info.push_str(&format!("total_cost_usd: ${:.4}\n", state.total_cost_usd));
    info
}

/// Submit feedback to Anthropic API via the CLI feedback endpoint.
async fn submit_feedback(
    description: &str,
    env_info: &str,
    api_key: &str,
) -> std::result::Result<String, String> {
    let report = serde_json::json!({
        "description": description,
        "datetime": chrono::Utc::now().to_rfc3339(),
        "platform": std::env::consts::OS,
        "version": format!("rclaude v{}", env!("CARGO_PKG_VERSION")),
        "environment": env_info,
    });

    let client = reqwest::Client::new();
    let resp = client
        .post("https://api.anthropic.com/api/claude_cli_feedback")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&serde_json::json!({ "content": report.to_string() }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|e| format!("Failed to submit: {e}"))?;

    let status = resp.status();
    if status.is_success() {
        let data: serde_json::Value = resp.json().await.unwrap_or_default();
        if let Some(id) = data.get("feedback_id").and_then(|v| v.as_str()) {
            return Ok(id.to_string());
        }
    }
    Err(format!("Server returned {status}"))
}

#[async_trait]
impl Command for FeedbackCommand {
    fn name(&self) -> &str {
        "feedback"
    }

    fn description(&self) -> &str {
        "Submit feedback about rclaude"
    }

    async fn execute(&self, args: &str, state: &mut AppState) -> Result<CommandResult> {
        let description = args.trim();

        if description.is_empty() {
            return Ok(CommandResult::Ok(Some(
                "Usage: /feedback <description>\nDescribe the bug or feature request.".to_string(),
            )));
        }

        let env_info = collect_env_info(state);
        let api_key = state
            .config
            .api_key
            .clone()
            .or_else(rclaude_core::auth::get_api_key)
            .unwrap_or_default();

        if api_key.is_empty() {
            return Ok(CommandResult::Ok(Some(
                "Cannot submit feedback: no API key configured.".to_string(),
            )));
        }

        eprintln!("{}", "Submitting feedback...".dimmed());

        match submit_feedback(description, &env_info, &api_key).await {
            Ok(id) => Ok(CommandResult::Ok(Some(format!(
                "{} Feedback submitted (ID: {id}). Thank you!",
                "✓".green()
            )))),
            Err(e) => Ok(CommandResult::Ok(Some(format!(
                "{} Failed to submit feedback: {e}",
                "✗".red()
            )))),
        }
    }
}
