//! Agent fork: create cache-sharing subagents.

use rclaude_api::types::{ApiContentBlock, ApiMessage};

/// Build forked messages for a child agent that shares the parent's prompt cache.
/// Creates placeholder tool_results so the API prefix is byte-identical.
pub fn build_forked_messages(
    parent_messages: &[ApiMessage],
    child_directive: &str,
) -> Vec<ApiMessage> {
    let mut messages = Vec::new();

    // Copy parent messages, replacing tool_result content with placeholders
    for msg in parent_messages {
        let content: Vec<ApiContentBlock> = msg
            .content
            .iter()
            .map(|block| match block {
                ApiContentBlock::ToolResult {
                    tool_use_id,
                    is_error,
                    ..
                } => ApiContentBlock::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    content: serde_json::Value::String("[forked — see parent]".into()),
                    is_error: *is_error,
                },
                other => other.clone(),
            })
            .collect();
        messages.push(ApiMessage {
            role: msg.role.clone(),
            content,
        });
    }

    // Add the child directive as a new user message
    messages.push(ApiMessage {
        role: "user".into(),
        content: vec![ApiContentBlock::Text {
            text: child_directive.to_string(),
        }],
    });

    messages
}

/// Build the child directive message with strict rules.
pub fn build_child_directive(task: &str, worktree_path: Option<&str>) -> String {
    let mut directive = format!(
        "You are a forked worker agent. Complete this task independently:\n\n\
         {task}\n\n\
         Rules:\n\
         1. Work only within the scope described above\n\
         2. Do NOT spawn sub-agents\n\
         3. Use absolute file paths\n\
         4. Commit your changes when done\n\
         5. Report results concisely"
    );

    if let Some(path) = worktree_path {
        directive.push_str(&format!(
            "\n\nYou are working in an isolated git worktree at: {path}\n\
             All file paths should be relative to this directory."
        ));
    }

    directive
}
