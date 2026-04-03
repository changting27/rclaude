//! Compact service: API-based conversation summarization with context recovery.
//! Q04: After compaction, restore recently-read files, plans, and skill instructions.

use rclaude_core::message::{ContentBlock, Message, Role};

const COMPACT_SYSTEM_PROMPT: &str = "CRITICAL: Respond with TEXT ONLY. Do NOT call any tools.\n\n\
Your task is to summarize the conversation so far. Provide a detailed summary that preserves:\n\
1. The user's original requests and goals\n\
2. Key decisions and approaches taken\n\
3. File names, code snippets, and function signatures\n\
4. Errors encountered and how they were resolved\n\
5. Current state of the task\n\n\
Format your response as:\n\
<summary>\n\
[Your detailed summary here]\n\
</summary>";

const COMPACT_USER_PROMPT: &str = "Please provide a detailed summary of our conversation so far. \
Focus on preserving all technical details, file paths, code changes, and the current state of the task.";

/// Max files to restore after compaction.
const POST_COMPACT_MAX_FILES: usize = 5;
/// Max tokens per restored file.
const POST_COMPACT_MAX_TOKENS_PER_FILE: usize = 5_000;
/// Total token budget for restored files.
const POST_COMPACT_TOKEN_BUDGET: usize = 50_000;

#[derive(Debug)]
pub struct CompactResult {
    pub summary: String,
    pub messages_before: usize,
    pub messages_after: usize,
    pub tokens_saved_estimate: usize,
}

/// Compact a conversation by calling the API to generate a summary.
pub async fn compact_conversation(
    messages: &[Message],
    api_key: &str,
    model: &str,
    keep_recent: usize,
) -> Result<CompactResult, String> {
    if messages.len() <= keep_recent * 2 {
        return Err("Not enough messages to compact".into());
    }

    let messages_before = messages.len();
    let cutoff = messages.len().saturating_sub(keep_recent * 2);
    let to_summarize = &messages[..cutoff];

    // Build condensed conversation for the API
    let mut conversation_text = String::new();
    for msg in to_summarize {
        let role = match msg.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
            Role::System => "System",
        };
        let text = msg.text_content();
        if !text.is_empty() {
            let truncated = if text.len() > 2000 {
                format!("{}... [truncated]", &text[..2000])
            } else {
                text
            };
            conversation_text.push_str(&format!("{role}: {truncated}\n\n"));
        }
    }

    let summary = call_api_for_summary(api_key, model, &conversation_text).await?;
    let tokens_saved = rclaude_core::context_window::estimate_conversation_tokens(to_summarize);

    Ok(CompactResult {
        summary,
        messages_before,
        messages_after: 1 + (messages.len() - cutoff),
        tokens_saved_estimate: tokens_saved,
    })
}

/// Build compacted messages with context recovery.
/// Q04: Restores recently-read files after compaction.
pub fn build_compacted_messages(
    messages: &[Message],
    summary: &str,
    keep_recent: usize,
) -> Vec<Message> {
    let mut result = Vec::new();

    // Keep system messages
    for msg in messages.iter() {
        if msg.role == Role::System {
            result.push(msg.clone());
        } else {
            break;
        }
    }

    // Add summary
    result.push(Message::assistant(vec![ContentBlock::Text {
        text: format!("[Conversation summary]\n\n{summary}"),
    }]));

    // Q04: Restore recently-read files from the conversation
    let restored = extract_recently_read_files(messages, keep_recent);
    if !restored.is_empty() {
        let mut restore_text = String::from("[Files from before compaction]\n\n");
        let mut budget_used = 0;
        for (path, content) in restored.iter().take(POST_COMPACT_MAX_FILES) {
            let tokens = content.len() / 4; // rough estimate
            if tokens > POST_COMPACT_MAX_TOKENS_PER_FILE {
                continue;
            }
            if budget_used + tokens > POST_COMPACT_TOKEN_BUDGET {
                break;
            }
            restore_text.push_str(&format!("File: {path}\n```\n{content}\n```\n\n"));
            budget_used += tokens;
        }
        result.push(Message::assistant(vec![ContentBlock::Text {
            text: restore_text,
        }]));
    }

    // Restore active plan if it exists
    if let Some(plan) = extract_active_plan(messages) {
        result.push(Message::assistant(vec![ContentBlock::Text {
            text: format!("[Active plan from before compaction]\n\n{plan}"),
        }]));
    }

    // Restore invoked skill instructions
    let skills = extract_invoked_skills(messages);
    if !skills.is_empty() {
        let mut skill_text = String::from("[Skills invoked before compaction]\n\n");
        for (name, instruction) in &skills {
            skill_text.push_str(&format!("### {name}\n{instruction}\n\n"));
        }
        result.push(Message::assistant(vec![ContentBlock::Text {
            text: skill_text,
        }]));
    }

    // Q04: Restore session memory summary if available
    if let Some(mem) = extract_session_memory(messages) {
        result.push(Message::assistant(vec![ContentBlock::Text {
            text: format!("[Session memory]\n\n{mem}"),
        }]));
    }

    // Keep recent messages
    let start = messages.len().saturating_sub(keep_recent * 2);
    result.extend_from_slice(&messages[start..]);

    result
}

/// Extract file paths and contents from Read tool results in the conversation.
fn extract_recently_read_files(messages: &[Message], keep_recent: usize) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = Vec::new();
    let cutoff = messages.len().saturating_sub(keep_recent * 2);

    // Scan messages being compacted for Read tool results
    // Look for patterns: tool_use with name "Read" followed by tool_result
    let mut pending_read_path: Option<String> = None;

    for msg in &messages[..cutoff] {
        for block in &msg.content {
            match block {
                ContentBlock::ToolUse { name, input, .. } if name == "Read" => {
                    pending_read_path = input
                        .get("file_path")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
                ContentBlock::ToolResult {
                    content, is_error, ..
                } if !is_error => {
                    if let Some(path) = pending_read_path.take() {
                        let text = content.as_str().unwrap_or("").to_string();
                        if !text.is_empty() && text.len() < POST_COMPACT_MAX_TOKENS_PER_FILE * 4 {
                            // Deduplicate: keep latest read of each file
                            files.retain(|(p, _)| p != &path);
                            files.push((path, text));
                        }
                    }
                }
                _ => {
                    pending_read_path = None;
                }
            }
        }
    }

    // Return most recent files first
    files.reverse();
    files
}

/// Extract the most recent active plan from EnterPlanMode/ExitPlanMode tool interactions.
fn extract_active_plan(messages: &[Message]) -> Option<String> {
    // Scan backwards for the last plan content
    for msg in messages.iter().rev() {
        for block in &msg.content {
            if let ContentBlock::ToolUse { name, input, .. } = block {
                if name == "EnterPlanMode" || name == "TodoWrite" {
                    if let Some(content) = input.get("content").and_then(|v| v.as_str()) {
                        if !content.is_empty() {
                            return Some(content.to_string());
                        }
                    }
                    if let Some(plan) = input.get("plan").and_then(|v| v.as_str()) {
                        if !plan.is_empty() {
                            return Some(plan.to_string());
                        }
                    }
                }
            }
        }
    }
    None
}

/// Extract skill instructions that were invoked during the conversation.
fn extract_invoked_skills(messages: &[Message]) -> Vec<(String, String)> {
    let mut skills: Vec<(String, String)> = Vec::new();
    for msg in messages {
        for block in &msg.content {
            if let ContentBlock::ToolUse { name, input, .. } = block {
                if name == "Skill" {
                    let skill_name = input
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    // Look for the skill result in subsequent tool_result
                    if let Some(instruction) = input.get("instruction").and_then(|v| v.as_str()) {
                        if !skills.iter().any(|(n, _)| n == skill_name) {
                            skills.push((skill_name.to_string(), instruction.to_string()));
                        }
                    }
                }
            }
        }
    }
    skills
}

/// Extract session memory facts from the conversation.
/// Looks for memory-related content that should survive compaction.
fn extract_session_memory(messages: &[Message]) -> Option<String> {
    let mut facts = Vec::new();
    for msg in messages {
        let text = msg.text_content();
        // Look for memory markers
        if text.contains("<session-memory>") {
            if let Some(start) = text.find("<session-memory>") {
                if let Some(end) = text.find("</session-memory>") {
                    let mem = &text[start + 16..end];
                    if !mem.trim().is_empty() {
                        facts.push(mem.trim().to_string());
                    }
                }
            }
        }
    }
    if facts.is_empty() {
        None
    } else {
        Some(facts.join("\n\n"))
    }
}
async fn call_api_for_summary(
    api_key: &str,
    model: &str,
    conversation: &str,
) -> Result<String, String> {
    let base_url = std::env::var("ANTHROPIC_BASE_URL")
        .unwrap_or_else(|_| "https://api.anthropic.com".to_string());

    let request = serde_json::json!({
        "model": model,
        "max_tokens": 4096,
        "system": COMPACT_SYSTEM_PROMPT,
        "messages": [{
            "role": "user",
            "content": format!("Here is the conversation to summarize:\n\n{conversation}\n\n{COMPACT_USER_PROMPT}")
        }]
    });

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base_url}/v1/messages"))
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Compact API call failed: {e}"))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("Compact API error: {body}"));
    }

    let response: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse compact response: {e}"))?;

    let text = response
        .get("content")
        .and_then(|c| c.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|b| {
                if b.get("type")?.as_str()? == "text" {
                    b.get("text")?.as_str().map(|s| s.to_string())
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| "Summary unavailable".to_string());

    // Extract content between <summary> tags if present
    if let Some(start) = text.find("<summary>") {
        if let Some(end) = text.find("</summary>") {
            return Ok(text[start + 9..end].trim().to_string());
        }
    }

    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_compacted_messages() {
        let msgs: Vec<Message> = (0..10)
            .map(|i| Message::user(format!("Message {i}")))
            .collect();
        let compacted = build_compacted_messages(&msgs, "This is a summary", 3);
        assert!(compacted
            .iter()
            .any(|m| m.text_content().contains("summary")));
        assert!(compacted
            .last()
            .unwrap()
            .text_content()
            .contains("Message 9"));
    }

    #[test]
    fn test_build_compacted_preserves_system() {
        let mut msgs = vec![Message {
            uuid: uuid::Uuid::new_v4(),
            role: Role::System,
            content: vec![ContentBlock::Text {
                text: "system".into(),
            }],
            timestamp: chrono::Utc::now(),
            model: None,
        }];
        msgs.extend((0..10).map(|i| Message::user(format!("msg {i}"))));
        let compacted = build_compacted_messages(&msgs, "summary", 3);
        assert_eq!(compacted[0].role, Role::System);
    }

    #[test]
    fn test_extract_recently_read_files() {
        let msgs = vec![
            Message::assistant(vec![ContentBlock::ToolUse {
                id: "1".into(),
                name: "Read".into(),
                input: serde_json::json!({"file_path": "src/main.rs"}),
            }]),
            Message {
                uuid: uuid::Uuid::new_v4(),
                role: Role::User,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "1".into(),
                    content: serde_json::Value::String("fn main() {}".into()),
                    is_error: false,
                }],
                timestamp: chrono::Utc::now(),
                model: None,
            },
            // Recent messages (won't be scanned)
            Message::user("recent 1"),
            Message::user("recent 2"),
        ];
        let files = extract_recently_read_files(&msgs, 1);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].0, "src/main.rs");
    }
}
