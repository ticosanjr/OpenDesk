//! Ports used by the queue runner to cross into a shell.
//!
//! The core owns claim/lease/execution. Delivery, learning, I/O sinks and task
//! autostart remain shell concerns and are injected here.

use crate::error::AppResult;
use opendesk_storage::events::EventBus;
use opendesk_storage::paths::AppPaths;
use opendesk_storage::repos::queue::QueueJob;

use async_trait::async_trait;
use sqlx::SqlitePool;

#[async_trait]
pub trait QueuePorts: Send + Sync {
    /// Best-effort work after settlement. Implementations should not block the
    /// worker for long; spawn network delivery when needed.
    async fn after_run(
        &self,
        pool: &SqlitePool,
        paths: &AppPaths,
        events: &EventBus,
        job: &QueueJob,
        run_id: &str,
        success: bool,
    ) -> AppResult<()>;
}

pub struct NoopQueuePorts;

#[async_trait]
impl QueuePorts for NoopQueuePorts {
    async fn after_run(
        &self,
        _pool: &SqlitePool,
        _paths: &AppPaths,
        _events: &EventBus,
        _job: &QueueJob,
        _run_id: &str,
        _success: bool,
    ) -> AppResult<()> {
        Ok(())
    }
}

#[async_trait]
pub trait TaskDispatcher: Send + Sync {
    async fn dispatch_task(&self, task_id: &str) -> AppResult<()>;
}

pub struct NoopTaskDispatcher;

#[async_trait]
impl TaskDispatcher for NoopTaskDispatcher {
    async fn dispatch_task(&self, _task_id: &str) -> AppResult<()> {
        Ok(())
    }
}
