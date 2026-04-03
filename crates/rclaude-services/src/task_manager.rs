//! Task management for tracking and organizing work items.
//! Manages background shell tasks and their lifecycle.

use std::collections::HashMap;
use std::path::PathBuf;

/// Background task state.
#[derive(Debug, Clone)]
pub struct BackgroundTask {
    pub id: String,
    pub description: String,
    pub command: String,
    pub cwd: PathBuf,
    pub status: TaskStatus,
    pub output_file: PathBuf,
    pub started_at: std::time::Instant,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Killed,
}

/// Task manager.
#[derive(Debug, Default)]
pub struct TaskManager {
    tasks: HashMap<String, BackgroundTask>,
    next_id: u32,
}

impl TaskManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new background task.
    pub fn create_task(
        &mut self,
        description: &str,
        command: &str,
        cwd: &std::path::Path,
    ) -> String {
        self.next_id += 1;
        let id = format!("task-{}", self.next_id);
        let output_dir = cwd.join(".claude/tasks");
        let _ = std::fs::create_dir_all(&output_dir);
        let output_file = output_dir.join(format!("{id}.log"));

        self.tasks.insert(
            id.clone(),
            BackgroundTask {
                id: id.clone(),
                description: description.to_string(),
                command: command.to_string(),
                cwd: cwd.to_path_buf(),
                status: TaskStatus::Pending,
                output_file,
                started_at: std::time::Instant::now(),
                pid: None,
            },
        );
        id
    }

    /// Update task status.
    pub fn update_status(&mut self, id: &str, status: TaskStatus) {
        if let Some(task) = self.tasks.get_mut(id) {
            task.status = status;
        }
    }

    /// Set task PID.
    pub fn set_pid(&mut self, id: &str, pid: u32) {
        if let Some(task) = self.tasks.get_mut(id) {
            task.pid = Some(pid);
        }
    }

    /// Get a task by ID.
    pub fn get_task(&self, id: &str) -> Option<&BackgroundTask> {
        self.tasks.get(id)
    }

    /// List all tasks.
    pub fn list_tasks(&self) -> Vec<&BackgroundTask> {
        self.tasks.values().collect()
    }

    /// List running tasks.
    pub fn running_tasks(&self) -> Vec<&BackgroundTask> {
        self.tasks
            .values()
            .filter(|t| t.status == TaskStatus::Running)
            .collect()
    }

    /// Remove completed tasks.
    pub fn cleanup_completed(&mut self) {
        self.tasks
            .retain(|_, t| t.status == TaskStatus::Running || t.status == TaskStatus::Pending);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_lifecycle() {
        let mut mgr = TaskManager::new();
        let id = mgr.create_task("test", "echo hi", std::path::Path::new("/tmp"));
        assert_eq!(mgr.get_task(&id).unwrap().status, TaskStatus::Pending);
        mgr.update_status(&id, TaskStatus::Running);
        assert_eq!(mgr.running_tasks().len(), 1);
        mgr.update_status(&id, TaskStatus::Completed);
        assert_eq!(mgr.running_tasks().len(), 0);
        mgr.cleanup_completed();
        assert!(mgr.list_tasks().is_empty());
    }
}
