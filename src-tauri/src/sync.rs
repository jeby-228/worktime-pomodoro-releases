use crate::store::{Event, TaskCache};
use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

/// Trait for syncing with remote backend
#[async_trait]
pub trait SyncTarget: Send + Sync {
    /// Fetch tasks from remote
    async fn fetch_tasks(&self, user: &str) -> Result<Vec<TaskCache>>;

    /// Push events to remote, returns UUIDs of successfully synced events
    async fn push_events(&self, events: &[Event]) -> Result<Vec<Uuid>>;

    /// Check if sync is available (network, credentials, etc.)
    async fn is_available(&self) -> bool;
}

/// No-op sync implementation for offline-only mode
pub struct NoOpSync;

#[async_trait]
impl SyncTarget for NoOpSync {
    async fn fetch_tasks(&self, _user: &str) -> Result<Vec<TaskCache>> {
        Ok(Vec::new())
    }

    async fn push_events(&self, _events: &[Event]) -> Result<Vec<Uuid>> {
        Ok(Vec::new())
    }

    async fn is_available(&self) -> bool {
        false
    }
}

/// Mock sync for testing
pub struct MockSync {
    pub tasks: Vec<TaskCache>,
    pub synced_events: Vec<Uuid>,
}

impl MockSync {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            synced_events: Vec::new(),
        }
    }

    pub fn with_tasks(tasks: Vec<TaskCache>) -> Self {
        Self {
            tasks,
            synced_events: Vec::new(),
        }
    }
}

impl Default for MockSync {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SyncTarget for MockSync {
    async fn fetch_tasks(&self, _user: &str) -> Result<Vec<TaskCache>> {
        Ok(self.tasks.clone())
    }

    async fn push_events(&self, events: &[Event]) -> Result<Vec<Uuid>> {
        let uuids: Vec<Uuid> = events.iter().map(|e| e.uuid).collect();
        Ok(uuids)
    }

    async fn is_available(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_noop_sync() {
        let sync = NoOpSync;
        assert!(!sync.is_available().await);

        let tasks = sync.fetch_tasks("test_user").await.unwrap();
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn test_mock_sync() {
        let sync = MockSync::new();
        assert!(sync.is_available().await);

        let tasks = sync.fetch_tasks("test_user").await.unwrap();
        assert!(tasks.is_empty());
    }
}
