use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;
pub struct FilesCommand;
#[async_trait]
impl Command for FilesCommand {
    fn name(&self) -> &str {
        "files"
    }
    fn description(&self) -> &str {
        "List files read/written in this session"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        // Extract file paths from tool_use/tool_result messages
        let mut files = std::collections::BTreeSet::new();
        for msg in &state.messages {
            for block in &msg.content {
                if let rclaude_core::message::ContentBlock::ToolUse { name, input, .. } = block {
                    if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                        let prefix = match name.as_str() {
                            "Read" => "R",
                            "Write" => "W",
                            "Edit" => "E",
                            _ => continue,
                        };
                        files.insert(format!("[{prefix}] {path}"));
                    }
                }
            }
        }
        if files.is_empty() {
            Ok(CommandResult::Ok(Some("No files in context.".into())))
        } else {
            Ok(CommandResult::Ok(Some(format!(
                "Files in context ({}):\n{}",
                files.len(),
                files.into_iter().collect::<Vec<_>>().join("\n")
            ))))
        }
    }
}
