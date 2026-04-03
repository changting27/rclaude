//! Auto-dream service matching services/autoDream/.
//! Background processing during idle periods.

use std::time::{Duration, Instant};

/// Auto-dream state.
pub struct AutoDreamState {
    last_activity: Instant,
    idle_threshold: Duration,
    enabled: bool,
    pending_tasks: Vec<DreamTask>,
}

#[derive(Debug, Clone)]
pub struct DreamTask {
    pub name: String,
    pub priority: u8,
}

impl AutoDreamState {
    pub fn new(idle_threshold: Duration) -> Self {
        Self {
            last_activity: Instant::now(),
            idle_threshold,
            enabled: true,
            pending_tasks: Vec::new(),
        }
    }

    /// Record user activity (resets idle timer).
    pub fn record_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Check if idle long enough to dream.
    pub fn should_dream(&self) -> bool {
        self.enabled
            && self.last_activity.elapsed() >= self.idle_threshold
            && !self.pending_tasks.is_empty()
    }

    /// Add a background task.
    pub fn add_task(&mut self, name: &str, priority: u8) {
        self.pending_tasks.push(DreamTask {
            name: name.to_string(),
            priority,
        });
        self.pending_tasks
            .sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Get next task to execute.
    pub fn next_task(&mut self) -> Option<DreamTask> {
        if self.pending_tasks.is_empty() {
            None
        } else {
            Some(self.pending_tasks.remove(0))
        }
    }

    pub fn pending_count(&self) -> usize {
        self.pending_tasks.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auto_dream() {
        let mut state = AutoDreamState::new(Duration::from_secs(0));
        state.add_task("memory_extract", 5);
        state.add_task("cleanup", 1);
        assert!(state.should_dream());
        let task = state.next_task().unwrap();
        assert_eq!(task.name, "memory_extract"); // higher priority first
    }
}
