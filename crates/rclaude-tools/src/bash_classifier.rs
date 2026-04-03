//! Bash LLM classifier for auto-mode permission decisions.
//! Uses a sideQuery to classify bash commands as allow/deny in auto mode.

use rclaude_core::error::Result;

const CLASSIFIER_SYSTEM_PROMPT: &str = "\
You are a security classifier for a CLI tool. Your job is to determine whether a bash command \
should be allowed to execute automatically (without user confirmation) in auto mode.\n\n\
Rules:\n\
- ALLOW: read-only commands, build/test commands, git read operations, package installs in project scope\n\
- DENY: destructive commands (rm -rf, git push --force), commands that modify system state, \
  commands that access sensitive data, network requests to unknown hosts, commands that could \
  leak credentials\n\n\
Respond with exactly one word: ALLOW or DENY";

const CLASSIFIER_MAX_TOKENS: u32 = 8;

/// Classify a bash command using LLM sideQuery.
/// Returns true if the command should be allowed.
pub async fn classify_bash_command(command: &str, api_key: &str, model: &str) -> Result<bool> {
    let base_url = std::env::var("ANTHROPIC_BASE_URL")
        .unwrap_or_else(|_| "https://api.anthropic.com".to_string());

    let request = serde_json::json!({
        "model": model,
        "max_tokens": CLASSIFIER_MAX_TOKENS,
        "system": CLASSIFIER_SYSTEM_PROMPT,
        "messages": [{
            "role": "user",
            "content": format!("Classify this command: {command}")
        }]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base_url}/v1/messages"))
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            let body: serde_json::Value = r.json().await.unwrap_or_default();
            let text = body
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|a| a.first())
                .and_then(|b| b.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("DENY");
            Ok(text.trim().to_uppercase().starts_with("ALLOW"))
        }
        _ => {
            // On error, fall back to deny (safe default)
            Ok(false)
        }
    }
}
