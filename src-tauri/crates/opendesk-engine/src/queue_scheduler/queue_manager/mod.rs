use crate::{
    adapters::AdapterService,
    error::{AppError, AppResult},
    notifier::RunNotifier,
    opencode_client::daemon_manager::DaemonManager,
    queue_ports::QueuePorts,
    queue_scheduler::{job_dto, leases},
};
use opendesk_storage::events::{EventBus, OpenDeskWsEvent, QueueJobDto};
use opendesk_storage::paths::AppPaths;
use opendesk_storage::repos::queue;

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

mod execute;
mod fallback;
mod worker;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStatus {
    pub paused: bool,
    pub pause_reason: Option<String>,
    pub active_jobs: usize,
    pub concurrency: usize,
    pub running: i64,
    pub waiting: i64,
}

pub struct QueueManager {
    pool: SqlitePool,
    scrubber: Arc<opendesk_storage::secrets::SecretScrubber>,
    paths: AppPaths,
    events: EventBus,
    write_buffer: Arc<opendesk_storage::write_buffer::WriteBuffer>,
    daemon: Arc<DaemonManager>,
    adapters: Arc<AdapterService>,
    ports: Arc<dyn QueuePorts>,
    notifier: Arc<dyn RunNotifier>,
    paused: AtomicBool,
    pause_reason: Mutex<Option<String>>,
    concurrency: AtomicUsize,
    active: AtomicUsize,
    notify: Notify,
    shutdown: CancellationToken,
    running_jobs: Mutex<HashMap<String, CancellationToken>>,
    lease_expired: Mutex<HashSet<String>>,
}

fn forces_worktree(tags_json: Option<&str>) -> bool {
    tags_json
        .and_then(|tags| serde_json::from_str::<Vec<String>>(tags).ok())
        .is_some_and(|tags| tags.iter().any(|tag| tag.starts_with("ab:")))
}

async fn worktree_runs_enabled(pool: &sqlx::SqlitePool) -> bool {
    opendesk_storage::repos::settings::get(pool, "worktree_runs")
        .await
        .ok()
        .flatten()
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

pub struct QueueConfig {
    pub pool: SqlitePool,
    pub scrubber: Arc<opendesk_storage::secrets::SecretScrubber>,
    pub paths: AppPaths,
    pub events: EventBus,
    pub write_buffer: Arc<opendesk_storage::write_buffer::WriteBuffer>,
    pub daemon: Arc<DaemonManager>,
    pub adapters: Arc<AdapterService>,
    pub ports: Arc<dyn QueuePorts>,
    pub notifier: Arc<dyn RunNotifier>,
    pub concurrency: usize,
}

impl QueueManager {
    pub fn new(config: QueueConfig) -> Arc<Self> {
        let QueueConfig {
            pool,
            scrubber,
            paths,
            events,
            write_buffer,
            daemon,
            adapters,
            ports,
            notifier,
            concurrency,
        } = config;
        Arc::new(Self {
            pool,
            scrubber,
            paths,
            events,
            write_buffer,
            daemon,
            adapters,
            ports,
            notifier,
            paused: AtomicBool::new(false),
            pause_reason: Mutex::new(None),
            concurrency: AtomicUsize::new(clamp_concurrency(concurrency)),
            active: AtomicUsize::new(0),
            notify: Notify::new(),
            shutdown: CancellationToken::new(),
            running_jobs: Mutex::new(HashMap::new()),
            lease_expired: Mutex::new(HashSet::new()),
        })
    }

    pub fn spawn_worker(self: &Arc<Self>) {
        let manager = Arc::clone(self);
        tokio::spawn(async move {
            manager.worker_loop().await;
        });
    }

    pub async fn shutdown(&self) {
        self.shutdown.cancel();
        let tokens = self
            .running_jobs
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for token in tokens {
            token.cancel();
        }
        self.notify.notify_waiters();

        leases::drain_running(&self.running_jobs).await;
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn concurrency(&self) -> usize {
        self.concurrency.load(Ordering::Relaxed)
    }

    pub fn set_concurrency(&self, value: usize) {
        self.concurrency
            .store(clamp_concurrency(value), Ordering::Relaxed);
        self.notify.notify_waiters();
    }

    pub async fn status(&self, workspace_id: Option<&str>) -> AppResult<QueueStatus> {
        let (running, waiting) = queue::counts(&self.pool, workspace_id).await?;
        Ok(QueueStatus {
            paused: self.is_paused(),
            pause_reason: self
                .pause_reason
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .clone(),
            active_jobs: self.active.load(Ordering::Relaxed),
            concurrency: self.concurrency(),
            running,
            waiting,
        })
    }

    pub async fn pause(&self, reason: impl Into<String>) {
        self.paused.store(true, Ordering::Relaxed);
        *self
            .pause_reason
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(reason.into());
        self.publish_status_for_all().await;
    }

    pub async fn resume(&self) -> AppResult<()> {
        self.paused.store(false, Ordering::Relaxed);
        *self
            .pause_reason
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = None;
        self.notify.notify_waiters();
        self.publish_status_for_all().await;
        Ok(())
    }

    pub async fn list(&self, workspace_id: Option<&str>) -> AppResult<Vec<QueueJobDto>> {
        let jobs = queue::list(&self.pool, workspace_id).await?;
        Ok(jobs.into_iter().map(job_dto).collect())
    }

    pub async fn enqueue(&self, input: queue::EnqueueInput) -> AppResult<queue::QueueJob> {
        let workspace_id = input.workspace_id.clone();
        let job = queue::enqueue(&self.pool, input).await?;
        self.publish_queue(&workspace_id).await;
        self.notify.notify_waiters();
        Ok(job)
    }

    pub async fn cancel(&self, id: &str) -> AppResult<queue::QueueJob> {
        let job = queue::get(&self.pool, id).await?;
        if job.status == "RUNNING" {
            let token = self
                .running_jobs
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .get(id)
                .cloned();
            if let Some(token) = token {
                token.cancel();
                self.publish_queue(&job.workspace_id).await;
                return Ok(job);
            }
            return Err(AppError::Conflict(
                "the runner for this job is not available".into(),
            ));
        }
        let job = queue::cancel(&self.pool, id).await?;
        self.publish_queue(&job.workspace_id).await;
        Ok(job)
    }

    pub async fn retry(&self, id: &str) -> AppResult<queue::QueueJob> {
        let job = queue::retry(&self.pool, id).await?;
        self.publish_queue(&job.workspace_id).await;
        self.notify.notify_waiters();
        Ok(job)
    }

    pub async fn update_priority(&self, id: &str, priority: i64) -> AppResult<queue::QueueJob> {
        let job = queue::update_priority(&self.pool, id, priority).await?;
        self.publish_queue(&job.workspace_id).await;
        Ok(job)
    }

    async fn publish_queue(&self, workspace_id: &str) {
        if let Ok(jobs) = queue::list(&self.pool, Some(workspace_id)).await {
            self.events.publish(OpenDeskWsEvent::QueueUpdated {
                workspace_id: workspace_id.to_string(),
                jobs: jobs.into_iter().map(job_dto).collect(),
            });
        }
    }

    async fn publish_status_for_all(&self) {
        let _ = self.status(None).await;
    }
}

fn clamp_concurrency(value: usize) -> usize {
    value.clamp(1, 4)
}

fn split_model(model: &str) -> (Option<String>, String) {
    let base = model.split('#').next().unwrap_or(model);
    match base.split_once('/') {
        Some((provider, id)) => (Some(provider.to_string()), id.to_string()),
        None => (None, base.to_string()),
    }
}
