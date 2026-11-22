pub use crate::scheduler::recurrence::next_occurrence;
use crate::{
    dsh_router,
    error::{AppError, AppResult},
    opencode_client::daemon_manager::DaemonManager,
    queue_ports::TaskDispatcher,
    queue_scheduler::QueueManager,
};
use opendesk_storage::events::{EventBus, OpenDeskWsEvent};
use opendesk_storage::paths::AppPaths;
use opendesk_storage::repos::schedules::{self, Schedule};

use chrono::{DateTime, Datelike, Timelike, Utc};
use chrono_tz::Tz;
use serde::Serialize;
use sqlx::SqlitePool;
use std::{
    path::{Component, Path, PathBuf},
    sync::{Arc, OnceLock},
    time::Duration,
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

mod recipe_runs;
mod tick;

#[cfg(test)]
mod tests;

pub struct CronEngine {
    pool: SqlitePool,
    #[allow(dead_code)]
    paths: AppPaths,
    events: EventBus,
    queue: Arc<QueueManager>,
    daemon: Arc<DaemonManager>,
    shutdown: CancellationToken,
    wake: Notify,
    /// Injected by the shell/recipes adapter; keeps the engine free of a
    /// recipes dependency.
    recipe_runner: OnceLock<Arc<dyn crate::scheduler::ScheduledRecipeRunner>>,
    dispatcher: Arc<dyn TaskDispatcher>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CronTickResult {
    pub checked: usize,
    pub enqueued: usize,
}

impl CronEngine {
    pub fn new(
        pool: SqlitePool,
        paths: AppPaths,
        events: EventBus,
        queue: Arc<QueueManager>,
        daemon: Arc<DaemonManager>,
        dispatcher: Arc<dyn TaskDispatcher>,
    ) -> Arc<Self> {
        Arc::new(Self {
            pool,
            paths,
            events,
            queue,
            daemon,
            dispatcher,
            shutdown: CancellationToken::new(),
            wake: Notify::new(),
            recipe_runner: OnceLock::new(),
        })
    }

    pub fn spawn(self: &Arc<Self>) {
        let engine = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = engine.shutdown.cancelled() => break,
                    _ = engine.wake.notified() => {}
                    // Safety net for wall-clock changes; normal wakes come from
                    // schedule creation/edition through `wake()`.
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {}
                }
                if let Err(error) = engine.tick_once().await {
                    tracing::warn!(error = %error, "cron tick failed");
                }
            }
        });
    }

    pub fn shutdown(&self) {
        self.shutdown.cancel();
    }

    /// Wakes the scheduler immediately after a schedule is created, edited or
    /// removed, so ticks are event-driven instead of polling every 15 seconds.
    pub fn wake(&self) {
        self.wake.notify_waiters();
    }

    pub fn attach_recipe_runner(&self, runner: Arc<dyn crate::scheduler::ScheduledRecipeRunner>) {
        let _ = self.recipe_runner.set(runner);
    }

    pub async fn run_now(&self, schedule_id: &str) -> AppResult<serde_json::Value> {
        let schedule = schedules::get(&self.pool, schedule_id).await?;
        let now = Utc::now();
        let result = if schedule.target_kind == "RECIPE" {
            let value = self
                .run_recipe_schedule(&schedule, &now.to_rfc3339())
                .await?;
            serde_json::json!({ "kind": "recipe", "run": value })
        } else {
            let job = self
                .enqueue_schedule(&schedule, &now.to_rfc3339(), now)
                .await?
                .ok_or_else(|| AppError::Conflict("this schedule slot is already queued".into()))?;
            serde_json::json!({ "kind": "queue", "job": job })
        };
        let _ = schedules::mark_run(&self.pool, &schedule.id, "QUEUED", &now.to_rfc3339()).await;
        if schedule.trigger_type == "CRON"
            && let Ok(next) = next_occurrence(&schedule.cron_expression, &schedule.timezone, now)
        {
            let _ =
                schedules::set_next_run(&self.pool, &schedule.id, Some(&next.to_rfc3339())).await;
        }
        self.events.publish(OpenDeskWsEvent::ScheduleUpdated {
            schedule_id: schedule.id,
            workspace_id: schedule.workspace_id,
        });
        Ok(result)
    }
}

pub fn render_output_folder(
    schedule: &Schedule,
    workspace_root: &Path,
    now: DateTime<Utc>,
) -> AppResult<PathBuf> {
    let timezone: Tz = schedule.timezone.parse().unwrap_or(chrono_tz::UTC);
    let local = now.with_timezone(&timezone);
    let slug = dsh_router::slugify(&schedule.name);
    let rendered = schedule
        .output_folder_template
        .replace("{YYYY}", &format!("{:04}", local.year()))
        .replace("{MM}", &format!("{:02}", local.month()))
        .replace("{DD}", &format!("{:02}", local.day()))
        .replace("{HH}", &format!("{:02}", local.hour()))
        .replace("{mm}", &format!("{:02}", local.minute()))
        .replace("{schedule_slug}", &slug)
        .replace("{schedule_id}", &schedule.id);

    let path = Path::new(&rendered);
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(AppError::BadRequest(
            "output_folder_template cannot contain '..'".into(),
        ));
    }

    if path.is_absolute() {
        if !path.starts_with(workspace_root) {
            return Err(AppError::BadRequest(
                "output_folder_template absolute path must stay inside the workspace".into(),
            ));
        }
        Ok(path.to_path_buf())
    } else {
        Ok(workspace_root.join(path))
    }
}
