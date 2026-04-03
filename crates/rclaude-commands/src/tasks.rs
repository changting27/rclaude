use async_trait::async_trait;
use rclaude_core::command::{Command, CommandResult};
use rclaude_core::error::Result;
use rclaude_core::state::AppState;

pub struct TasksCommand;

#[async_trait]
impl Command for TasksCommand {
    fn name(&self) -> &str {
        "tasks"
    }
    fn description(&self) -> &str {
        "List background tasks"
    }
    async fn execute(&self, _args: &str, state: &mut AppState) -> Result<CommandResult> {
        if state.tasks.is_empty() {
            return Ok(CommandResult::Ok(Some("No background tasks.".into())));
        }
        let mut lines = vec![format!("{} task(s):", state.tasks.len())];
        for task in &state.tasks {
            let status = match task.status {
                rclaude_core::task::TaskStatus::Pending => "⏸ pending",
                rclaude_core::task::TaskStatus::Running => "⏳ running",
                rclaude_core::task::TaskStatus::Completed => "✅ done",
                rclaude_core::task::TaskStatus::Failed => "❌ failed",
                rclaude_core::task::TaskStatus::Killed => "⊘ killed",
            };
            lines.push(format!("  {} [{}] {}", task.id, status, task.description));
        }
        Ok(CommandResult::Ok(Some(lines.join("\n"))))
    }
}
