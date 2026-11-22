//! Shell implementations of the queue ports (A4).
//!
//! The core owns claim/lease/execution; this module keeps delivery, learning,
//! I/O sinks and task autostart in the shell.

use crate::{
    app_state::AppState,
    db::queue::QueueJob,
    integrations::{delivery, google_oauth::GoogleOAuth},
    queue_ports::{QueuePorts, TaskDispatcher},
    run_notifier::ShellRunNotifier,
};
use async_trait::async_trait;
use sqlx::SqlitePool;
use std::sync::{Arc, OnceLock, Weak};

pub struct ShellQueuePorts {
    pub google: Arc<GoogleOAuth>,
}

#[async_trait]
impl QueuePorts for ShellQueuePorts {
    async fn after_run(
        &self,
        pool: &SqlitePool,
        paths: &crate::paths::AppPaths,
        events: &crate::events::EventBus,
        job: &QueueJob,
        run_id: &str,
        success: bool,
    ) -> opendesk_engine::error::AppResult<()> {
        if let Ok(Some(lesson)) = crate::learning::capture_run(pool, run_id).await {
            tracing::debug!(lesson_id = %lesson.id, "learning lesson captured");
            events.publish(crate::events::OpenDeskWsEvent::LearningUpdated {
                workspace_id: job.workspace_id.clone(),
            });
        }
        // Delivery and sinks can be slow (network, rsync, Google). Spawn them
        // so the queue worker can move to the next job immediately.
        let pool = pool.clone();
        let paths = paths.clone();
        let job = job.clone();
        let run_id = run_id.to_string();
        let google = self.google.clone();
        tokio::spawn(async move {
            let io_ctx = crate::io_bus::IoContext {
                pool: pool.clone(),
                paths: paths.clone(),
            };
            if let Err(error) = delivery::deliver_run(
                &pool,
                &paths,
                &job.workspace_id,
                job.task_id.as_deref(),
                job.schedule_id.as_deref(),
                &run_id,
                google,
            )
            .await
            {
                tracing::warn!(error = %error, "post-run delivery failed");
            }
            if success
                && let Err(error) = crate::io_bus::run_sinks(
                    &io_ctx,
                    &job.workspace_id,
                    job.task_id.as_deref(),
                    job.schedule_id.as_deref(),
                    &run_id,
                )
                .await
            {
                tracing::warn!(error = %error, "post-run io sinks failed");
            }
        });
        Ok(())
    }
}

pub struct ShellTaskDispatcher {
    state: OnceLock<Weak<AppState>>,
}

impl Default for ShellTaskDispatcher {
    fn default() -> Self {
        Self {
            state: OnceLock::new(),
        }
    }
}

impl ShellTaskDispatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn attach(&self, state: &Arc<AppState>) {
        let _ = self.state.set(Arc::downgrade(state));
    }
}

#[async_trait]
impl TaskDispatcher for ShellTaskDispatcher {
    async fn dispatch_task(&self, task_id: &str) -> opendesk_engine::error::AppResult<()> {
        let state = self
            .state
            .get()
            .and_then(Weak::upgrade)
            .ok_or_else(|| crate::error::AppError::Internal("app state indisponível".into()))?;
        crate::api::tasks::dispatch_task_internal(
            &state,
            task_id,
            crate::api::tasks::DispatchBody::default(),
            None,
        )
        .await
        .map(|_| ())
        .map_err(Into::into)
    }
}

pub fn shell_run_notifier() -> Arc<dyn crate::notifier::RunNotifier> {
    Arc::new(ShellRunNotifier)
}
