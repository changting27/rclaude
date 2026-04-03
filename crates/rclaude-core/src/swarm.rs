//! Swarm multi-agent coordination matching utils/swarm/.
//! Manages multiple agent instances working together.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Teammate state.
#[derive(Debug, Clone)]
pub struct TeammateState {
    pub id: String,
    pub agent_type: String,
    pub status: TeammateStatus,
    pub task: String,
    pub messages_sent: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeammateStatus {
    Starting,
    Running,
    Idle,
    Stopped,
    Error,
}

/// Swarm coordinator manages multiple teammates.
pub struct SwarmCoordinator {
    teammates: Arc<RwLock<HashMap<String, TeammateState>>>,
    max_teammates: usize,
}

impl SwarmCoordinator {
    pub fn new(max_teammates: usize) -> Self {
        Self {
            teammates: Arc::new(RwLock::new(HashMap::new())),
            max_teammates,
        }
    }

    /// Spawn a new teammate.
    pub async fn spawn_teammate(
        &self,
        id: &str,
        agent_type: &str,
        task: &str,
    ) -> Result<(), String> {
        let mut teammates = self.teammates.write().await;
        if teammates.len() >= self.max_teammates {
            return Err(format!("Max teammates ({}) reached", self.max_teammates));
        }
        teammates.insert(
            id.to_string(),
            TeammateState {
                id: id.to_string(),
                agent_type: agent_type.to_string(),
                status: TeammateStatus::Starting,
                task: task.to_string(),
                messages_sent: 0,
            },
        );
        Ok(())
    }

    /// Update teammate status.
    pub async fn update_status(&self, id: &str, status: TeammateStatus) {
        let mut teammates = self.teammates.write().await;
        if let Some(t) = teammates.get_mut(id) {
            t.status = status;
        }
    }

    /// Remove a teammate.
    pub async fn remove_teammate(&self, id: &str) {
        self.teammates.write().await.remove(id);
    }

    /// Get all teammates.
    pub async fn list_teammates(&self) -> Vec<TeammateState> {
        self.teammates.read().await.values().cloned().collect()
    }

    /// Get active teammate count.
    pub async fn active_count(&self) -> usize {
        self.teammates
            .read()
            .await
            .values()
            .filter(|t| t.status == TeammateStatus::Running || t.status == TeammateStatus::Starting)
            .count()
    }

    /// Stop all teammates.
    pub async fn stop_all(&self) {
        let mut teammates = self.teammates.write().await;
        for t in teammates.values_mut() {
            t.status = TeammateStatus::Stopped;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spawn_and_list() {
        let coord = SwarmCoordinator::new(5);
        coord
            .spawn_teammate("t1", "general-purpose", "research")
            .await
            .unwrap();
        coord
            .spawn_teammate("t2", "Explore", "search")
            .await
            .unwrap();
        assert_eq!(coord.list_teammates().await.len(), 2);
    }

    #[tokio::test]
    async fn test_max_teammates() {
        let coord = SwarmCoordinator::new(1);
        coord
            .spawn_teammate("t1", "general-purpose", "task1")
            .await
            .unwrap();
        assert!(coord
            .spawn_teammate("t2", "general-purpose", "task2")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn test_stop_all() {
        let coord = SwarmCoordinator::new(5);
        coord
            .spawn_teammate("t1", "general-purpose", "task")
            .await
            .unwrap();
        coord.update_status("t1", TeammateStatus::Running).await;
        assert_eq!(coord.active_count().await, 1);
        coord.stop_all().await;
        assert_eq!(coord.active_count().await, 0);
    }
}
