//! Graceful shutdown handler.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Notify;

static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

/// Check if shutdown is in progress.
pub fn is_shutting_down() -> bool {
    SHUTTING_DOWN.load(Ordering::SeqCst)
}

/// Cleanup function type.
pub type CleanupFn = Box<dyn FnOnce() + Send>;

/// Graceful shutdown manager.
pub struct ShutdownManager {
    cleanup_fns: Vec<CleanupFn>,
    notify: Arc<Notify>,
}

impl ShutdownManager {
    pub fn new() -> Self {
        Self {
            cleanup_fns: Vec::new(),
            notify: Arc::new(Notify::new()),
        }
    }

    /// Register a cleanup function to run on shutdown.
    pub fn register_cleanup(&mut self, f: CleanupFn) {
        self.cleanup_fns.push(f);
    }

    /// Execute graceful shutdown with timeout.
    pub async fn shutdown(&mut self, timeout_ms: u64) {
        if SHUTTING_DOWN.swap(true, Ordering::SeqCst) {
            return; // Already shutting down
        }

        // Run cleanup functions with timeout
        let cleanup_fns = std::mem::take(&mut self.cleanup_fns);
        let cleanup = async {
            for f in cleanup_fns {
                f();
            }
        };

        match tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), cleanup).await {
            Ok(()) => tracing::debug!("Cleanup completed"),
            Err(_) => tracing::warn!("Cleanup timed out after {timeout_ms}ms"),
        }

        self.notify.notify_waiters();
    }

    /// Wait for shutdown to complete.
    pub async fn wait(&self) {
        self.notify.notified().await;
    }

    /// Reset shutdown state (for testing).
    pub fn reset() {
        SHUTTING_DOWN.store(false, Ordering::SeqCst);
    }
}

impl Default for ShutdownManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_shutdown_lifecycle() {
        // Test full lifecycle in a single test to avoid global state races
        ShutdownManager::reset();
        assert!(!is_shutting_down());

        // Test cleanup execution
        let mut mgr = ShutdownManager::new();
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        mgr.register_cleanup(Box::new(move || {
            called_clone.store(true, Ordering::SeqCst);
        }));
        mgr.shutdown(5000).await;
        assert!(is_shutting_down());
        assert!(called.load(Ordering::SeqCst));

        // Test double shutdown is no-op
        let mut mgr2 = ShutdownManager::new();
        mgr2.shutdown(1000).await; // Should be no-op (already shutting down)

        ShutdownManager::reset();
        assert!(!is_shutting_down());
    }
}
