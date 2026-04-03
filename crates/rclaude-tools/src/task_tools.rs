//! Task management tools with real AppState persistence.

use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;

use rclaude_core::error::{RclaudeError, Result};
use rclaude_core::task::{TaskState, TaskStatus, TaskType};
use rclaude_core::tool::{Tool, ToolInputSchema, ToolResult, ToolUseContext};

async fn with_state<F, R>(ctx: &ToolUseContext, f: F) -> Result<R>
where
    F: FnOnce(&mut rclaude_core::state::AppState) -> R,
{
    let state = ctx
        .app_state
        .as_ref()
        .ok_or_else(|| RclaudeError::Tool("No app state available".into()))?;
    let mut s = state.write().await;
    Ok(f(&mut s))
}

async fn read_state<F, R>(ctx: &ToolUseContext, f: F) -> Result<R>
where
    F: FnOnce(&rclaude_core::state::AppState) -> R,
{
    let state = ctx
        .app_state
        .as_ref()
        .ok_or_else(|| RclaudeError::Tool("No app state available".into()))?;
    let s = state.read().await;
    Ok(f(&s))
}

// ============================================================================
// TaskCreateTool
// ============================================================================

pub struct TaskCreateTool;

#[async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &str {
        "TaskCreate"
    }
    fn description(&self) -> &str {
        "Create a structured task for tracking progress."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "subject": { "type": "string", "description": "Brief task title" },
                "description": { "type": "string", "description": "What needs to be done" },
                "activeForm": { "type": "string", "description": "Present continuous form for spinner" }
            },
            "required": ["subject", "description"]
        })).expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let subject = input
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled");
        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let id = with_state(ctx, |s| {
            let seq = s.next_task_seq;
            s.next_task_seq += 1;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let task = TaskState {
                id: format!("{seq}"),
                task_type: TaskType::LocalAgent,
                status: TaskStatus::Pending,
                description: format!("{subject}: {description}"),
                start_time: now,
                end_time: None,
                output_file: PathBuf::new(),
                output_offset: 0,
                notified: false,
            };
            s.tasks.push(task);
            seq
        })
        .await?;

        Ok(ToolResult::text(format!(
            "Task #{id} created successfully: {subject}"
        )))
    }
}

// ============================================================================
// TaskListTool
// ============================================================================

pub struct TaskListTool;

#[async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &str {
        "TaskList"
    }
    fn description(&self) -> &str {
        "List all tasks."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({"type": "object", "properties": {}})).expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
    async fn execute(&self, _input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let output = read_state(ctx, |s| {
            if s.tasks.is_empty() {
                return "No tasks.".to_string();
            }
            s.tasks
                .iter()
                .map(|t| {
                    format!(
                        "#{} [{}] {}",
                        t.id,
                        format!("{:?}", t.status).to_lowercase(),
                        t.description
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .await?;
        Ok(ToolResult::text(output))
    }
}

// ============================================================================
// TaskUpdateTool
// ============================================================================

pub struct TaskUpdateTool;

#[async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &str {
        "TaskUpdate"
    }
    fn description(&self) -> &str {
        "Update a task's status."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "taskId": { "type": "string" },
                "status": { "type": "string" },
                "subject": { "type": "string" },
                "description": { "type": "string" }
            },
            "required": ["taskId"]
        }))
        .expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let task_id = input.get("taskId").and_then(|v| v.as_str()).unwrap_or("?");
        let new_status = input.get("status").and_then(|v| v.as_str());

        let msg = with_state(ctx, |s| {
            if let Some(task) = s.tasks.iter_mut().find(|t| t.id == task_id) {
                if let Some(status_str) = new_status {
                    task.status = match status_str {
                        "in_progress" => TaskStatus::Running,
                        "completed" => TaskStatus::Completed,
                        "failed" => TaskStatus::Failed,
                        "killed" | "deleted" => TaskStatus::Killed,
                        _ => task.status,
                    };
                }
                if let Some(desc) = input.get("description").and_then(|v| v.as_str()) {
                    task.description = desc.to_string();
                }
                format!("Updated task #{task_id} status")
            } else {
                format!("Task #{task_id} not found")
            }
        })
        .await?;
        Ok(ToolResult::text(msg))
    }
}

// ============================================================================
// TaskGetTool
// ============================================================================

pub struct TaskGetTool;

#[async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &str {
        "TaskGet"
    }
    fn description(&self) -> &str {
        "Retrieve a task by ID."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": { "taskId": { "type": "string" } },
            "required": ["taskId"]
        }))
        .expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let task_id = input.get("taskId").and_then(|v| v.as_str()).unwrap_or("?");
        let output = read_state(ctx, |s| match s.tasks.iter().find(|t| t.id == task_id) {
            Some(t) => format!("#{} [{:?}] {}", t.id, t.status, t.description),
            None => format!("Task #{task_id} not found"),
        })
        .await?;
        Ok(ToolResult::text(output))
    }
}

// ============================================================================
// TaskOutputTool
// ============================================================================

pub struct TaskOutputTool;

#[async_trait]
impl Tool for TaskOutputTool {
    fn name(&self) -> &str {
        "TaskOutput"
    }
    fn description(&self) -> &str {
        "Retrieve output from a background task."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": {
                "task_id": { "type": "string" },
                "block": { "type": "boolean" },
                "timeout": { "type": "number" }
            },
            "required": ["task_id", "block", "timeout"]
        }))
        .expect("valid schema")
    }

    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let task_id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("?");
        let output_file = read_state(ctx, |s| {
            s.tasks
                .iter()
                .find(|t| t.id == task_id)
                .map(|t| t.output_file.clone())
        })
        .await?;

        match output_file {
            Some(path) if path.exists() => {
                let content = tokio::fs::read_to_string(&path).await?;
                Ok(ToolResult::text(content))
            }
            _ => Ok(ToolResult::error(format!("No output for task {task_id}"))),
        }
    }
}

// ============================================================================
// TaskStopTool
// ============================================================================

pub struct TaskStopTool;

#[async_trait]
impl Tool for TaskStopTool {
    fn name(&self) -> &str {
        "TaskStop"
    }
    fn description(&self) -> &str {
        "Stop a running background task."
    }
    fn input_schema(&self) -> ToolInputSchema {
        serde_json::from_value(json!({
            "type": "object",
            "properties": { "task_id": { "type": "string" } },
            "required": ["task_id"]
        }))
        .expect("valid schema")
    }

    fn is_concurrency_safe(&self) -> bool {
        true
    }
    async fn execute(&self, input: Value, ctx: &ToolUseContext) -> Result<ToolResult> {
        let task_id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("?");
        let msg = with_state(ctx, |s| {
            if let Some(task) = s.tasks.iter_mut().find(|t| t.id == task_id) {
                task.status = TaskStatus::Killed;
                format!("Task {task_id} stopped")
            } else {
                format!("Task {task_id} not found")
            }
        })
        .await?;
        Ok(ToolResult::text(msg))
    }
}
