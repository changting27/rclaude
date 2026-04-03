//! Cron scheduler for periodic background tasks.
//! Manages recurring background tasks.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A scheduled cron task.
#[derive(Debug, Clone)]
pub struct CronTask {
    pub id: String,
    pub name: String,
    pub command: String,
    pub interval: Duration,
    pub last_run: Option<Instant>,
    pub enabled: bool,
}

/// Cron scheduler manages recurring tasks.
#[derive(Debug, Default)]
pub struct CronScheduler {
    tasks: HashMap<String, CronTask>,
}

impl CronScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new cron task.
    pub fn add_task(&mut self, id: &str, name: &str, command: &str, interval: Duration) {
        self.tasks.insert(
            id.to_string(),
            CronTask {
                id: id.to_string(),
                name: name.to_string(),
                command: command.to_string(),
                interval,
                last_run: None,
                enabled: true,
            },
        );
    }

    /// Remove a task.
    pub fn remove_task(&mut self, id: &str) -> bool {
        self.tasks.remove(id).is_some()
    }

    /// Get tasks that are due to run.
    pub fn due_tasks(&self) -> Vec<&CronTask> {
        self.tasks
            .values()
            .filter(|t| t.enabled && t.last_run.is_none_or(|lr| lr.elapsed() >= t.interval))
            .collect()
    }

    /// Mark a task as having just run.
    pub fn mark_run(&mut self, id: &str) {
        if let Some(task) = self.tasks.get_mut(id) {
            task.last_run = Some(Instant::now());
        }
    }

    /// List all tasks.
    pub fn list_tasks(&self) -> Vec<&CronTask> {
        self.tasks.values().collect()
    }

    /// Get task count.
    pub fn count(&self) -> usize {
        self.tasks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_list() {
        let mut sched = CronScheduler::new();
        sched.add_task("t1", "Test", "echo hi", Duration::from_secs(60));
        assert_eq!(sched.count(), 1);
        assert_eq!(sched.list_tasks().len(), 1);
    }

    #[test]
    fn test_due_tasks() {
        let mut sched = CronScheduler::new();
        sched.add_task("t1", "Test", "echo hi", Duration::from_secs(0));
        assert_eq!(sched.due_tasks().len(), 1); // never run = due
        sched.mark_run("t1");
        // Just ran, so with 0 interval it's immediately due again
    }

    #[test]
    fn test_remove() {
        let mut sched = CronScheduler::new();
        sched.add_task("t1", "Test", "echo hi", Duration::from_secs(60));
        assert!(sched.remove_task("t1"));
        assert!(!sched.remove_task("t1"));
        assert_eq!(sched.count(), 0);
    }
}
