use crate::{
    adapters::AdapterService,
    db::{budget, model_health, queue, router as router_db, runs, tasks, workspaces},
    error::{AppError, AppResult},
    events::{EventBus, OpenDeskWsEvent, QueueJobDto},
    integrations::{delivery, google_oauth::GoogleOAuth},
    opencode_client::{
        daemon_manager::DaemonManager,
        session_runner::{self, RunRequest, RunStatus},
    },
    paths::AppPaths,
    queue_scheduler::{run_execution, settlement},
    secrets::SecretStore,
};
use serde::Serialize;
use sqlx::SqlitePool;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Instant,
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize)]
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
    paths: AppPaths,
    events: EventBus,
    daemon: Arc<DaemonManager>,
    adapters: Arc<AdapterService>,
    google: Arc<GoogleOAuth>,
    paused: AtomicBool,
    pause_reason: Mutex<Option<String>>,
    concurrency: AtomicUsize,
    active: AtomicUsize,
    notify: Notify,
    shutdown: CancellationToken,
    running_jobs: Mutex<HashMap<String, CancellationToken>>,
    lease_expired: Mutex<HashSet<String>>,
}

struct HeartbeatGuard {
    stop: CancellationToken,
    handle: tauri::async_runtime::JoinHandle<()>,
}

impl Drop for HeartbeatGuard {
    fn drop(&mut self) {
        self.stop.cancel();
        self.handle.abort();
    }
}

impl QueueManager {
    pub fn new(
        pool: SqlitePool,
        paths: AppPaths,
        events: EventBus,
        daemon: Arc<DaemonManager>,
        google: Arc<GoogleOAuth>,
        concurrency: usize,
        adapters: Arc<AdapterService>,
    ) -> Arc<Self> {
        Arc::new(Self {
            pool,
            paths,
            events,
            daemon,
            adapters,
            google,
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
        tauri::async_runtime::spawn(async move {
            manager.worker_loop().await;
        });
    }

    pub async fn shutdown(&self) {
        self.shutdown.cancel();
        let tokens = self
            .running_jobs
            .lock()
            .expect("running jobs mutex")
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for token in tokens {
            token.cancel();
        }
        self.notify.notify_waiters();
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
                .expect("pause reason mutex")
                .clone(),
            active_jobs: self.active.load(Ordering::Relaxed),
            concurrency: self.concurrency(),
            running,
            waiting,
        })
    }

    pub async fn pause(&self, reason: impl Into<String>) {
        self.paused.store(true, Ordering::Relaxed);
        *self.pause_reason.lock().expect("pause reason mutex") = Some(reason.into());
        self.publish_status_for_all().await;
    }

    pub async fn resume(&self) -> AppResult<()> {
        self.paused.store(false, Ordering::Relaxed);
        *self.pause_reason.lock().expect("pause reason mutex") = None;
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
                .expect("running jobs mutex")
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

    async fn worker_loop(self: Arc<Self>) {
        if let Err(error) = self.recover().await {
            tracing::error!(error = %error, "queue recovery failed");
        }

        loop {
            tokio::select! {
                _ = self.shutdown.cancelled() => break,
                _ = self.notify.notified() => {}
                _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {}
            }

            self.expire_leases().await;

            if self.shutdown.is_cancelled() || self.is_paused() {
                continue;
            }

            // Fill every available concurrency slot before going back to
            // sleep. Each job runs in its own task so a slow model call does
            // not block the next queued job.
            loop {
                if self.active.load(Ordering::Relaxed) >= self.concurrency() {
                    break;
                }

                match budget::check(&self.pool).await {
                    Ok(check) if !check.allowed => {
                        self.pause(check.message.clone()).await;
                        self.events.publish(OpenDeskWsEvent::BudgetAlert {
                            level: check.level.into(),
                            daily_spent: check.status.daily_spent,
                            daily_limit: check.status.daily_limit,
                            message: check.message,
                        });
                        break;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        tracing::warn!(error = %error, "could not check budget");
                    }
                }

                let Some(job) = (match queue::next_queued(&self.pool).await {
                    Ok(job) => job,
                    Err(error) => {
                        tracing::error!(error = %error, "could not read queue");
                        None
                    }
                }) else {
                    break;
                };

                if !queue::claim(&self.pool, &job.id).await.unwrap_or(false) {
                    continue;
                }

                let workspace_id = job.workspace_id.clone();
                let manager = Arc::clone(&self);
                let active = self.active.fetch_add(1, Ordering::Relaxed) + 1;
                self.daemon.set_active_jobs(active).await;
                tauri::async_runtime::spawn(async move {
                    manager.execute_job(job).await;
                    let remaining = manager
                        .active
                        .fetch_sub(1, Ordering::Relaxed)
                        .saturating_sub(1);
                    manager.daemon.set_active_jobs(remaining).await;
                    manager.publish_queue(&workspace_id).await;
                    manager.notify.notify_waiters();
                });
            }
        }
    }

    async fn expire_leases(&self) {
        let Ok(jobs) = queue::expired_leases(&self.pool).await else {
            return;
        };
        for job in jobs {
            self.lease_expired
                .lock()
                .expect("lease set mutex")
                .insert(job.id.clone());
            if let Some(token) = self
                .running_jobs
                .lock()
                .expect("running jobs mutex")
                .get(&job.id)
            {
                token.cancel();
            }
            tracing::warn!(job = %job.id, "queue lease expired; interrupting job");
        }
    }

    async fn recover(&self) -> AppResult<()> {
        let runs = runs::recover_running(&self.pool).await?;
        let jobs = queue::recover_running(&self.pool).await?;
        if runs > 0 || jobs > 0 {
            let _ = sqlx::query(
                "UPDATE tasks SET status = 'BLOCKED', phase_status = 'failed',
                    blocked_reason = 'Execução interrompida por reinício do OpenDesk',
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id IN (
                    SELECT task_id FROM execution_queue
                    WHERE task_id IS NOT NULL
                      AND error_message LIKE 'interrupted by OpenDesk restart%'
                 )",
            )
            .execute(&self.pool)
            .await;
            tracing::warn!(
                runs,
                jobs,
                "recovered interrupted jobs after restart; manual retry required"
            );
        }
        self.notify.notify_waiters();
        Ok(())
    }

    async fn execute_job(&self, job: queue::QueueJob) {
        let workspace = match workspaces::get(&self.pool, &job.workspace_id).await {
            Ok(workspace) => workspace,
            Err(error) => {
                let _ =
                    queue::finish(&self.pool, &job.id, "FAILED", Some(&error.to_string())).await;
                settlement::mark_schedule(&self.pool, &self.events, &job, "FAILED").await;
                self.publish_queue(&job.workspace_id).await;
                return;
            }
        };

        let task = match &job.task_id {
            Some(task_id) => tasks::get(&self.pool, task_id).await.ok(),
            None => None,
        };

        let heartbeat_stop = CancellationToken::new();
        let heartbeat = {
            let pool = self.pool.clone();
            let job_id = job.id.clone();
            let stop = heartbeat_stop.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::select! {
                        _ = stop.cancelled() => break,
                        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                            let _ = queue::heartbeat(&pool, &job_id).await;
                        }
                    }
                }
            })
        };
        let _heartbeat_guard = HeartbeatGuard {
            stop: heartbeat_stop,
            handle: heartbeat,
        };

        let workspace_path = PathBuf::from(&workspace.root_path);
        let plan = run_execution::plan_run(&job, task.as_ref(), &self.paths, &workspace_path);
        let run_id = plan.run_id;
        let log_path = plan.log_path;
        let phase = plan.phase;
        let title = plan.title;
        let output_dir = plan.output_dir;
        let (provider_id, model_id) = split_model(&job.resolved_model);
        if let Err(error) = std::fs::create_dir_all(&output_dir) {
            tracing::warn!(path = %output_dir.display(), error = %error, "could not create output directory");
        }

        if let Some(task) = &task {
            let _ =
                tasks::set_phase_status(&self.pool, &task.id, Some("running"), Some("IN_PROGRESS"))
                    .await;
            self.events.publish(OpenDeskWsEvent::TaskUpdated {
                task_id: task.id.clone(),
                workspace_id: task.workspace_id.clone(),
                stage: "IN_PROGRESS".into(),
                phase_status: Some("running".into()),
            });
        }

        let parent_run_id = if job.session_id.is_some() {
            match job.task_id.as_deref() {
                Some(task_id) => runs::latest_for_task(&self.pool, task_id)
                    .await
                    .ok()
                    .flatten()
                    .map(|run| run.id),
                None => None,
            }
        } else {
            None
        };
        let run = match runs::create(
            &self.pool,
            runs::CreateRunInput {
                id: run_id.clone(),
                queue_job_id: Some(job.id.clone()),
                workspace_id: job.workspace_id.clone(),
                task_id: job.task_id.clone(),
                schedule_id: job.schedule_id.clone(),
                parent_run_id,
                phase: Some(phase.to_string()),
                agent_name: job.resolved_agent.clone(),
                execution_mode: job.resolved_mode.clone(),
                provider_id,
                model_id: model_id.clone(),
                effort_level: job.resolved_effort.clone(),
                pricing_snapshot_json: None,
                log_path: Some(log_path.to_string_lossy().to_string()),
            },
        )
        .await
        {
            Ok(run) => run,
            Err(error) => {
                let _ =
                    queue::finish(&self.pool, &job.id, "FAILED", Some(&error.to_string())).await;
                settlement::mark_schedule(&self.pool, &self.events, &job, "FAILED").await;
                self.publish_queue(&job.workspace_id).await;
                return;
            }
        };

        let rest = if job.resolved_harness == "opencode" {
            match self.daemon.client().await {
                Some(rest) => Some(rest),
                None => {
                    let _ = runs::finish(
                        &self.pool,
                        &run.id,
                        "FAILED",
                        &runs::RunUsage::default(),
                        0,
                        Some("OpenCode daemon is offline"),
                        None,
                    )
                    .await;
                    let _ = runs::set_error(
                        &self.pool,
                        &run.id,
                        "engine_offline",
                        "OpenCode daemon is offline",
                        "Abra Configurações → OpenCode Engine, configure o binário ou reinicie o motor.",
                    )
                    .await;
                    let _ = queue::finish(
                        &self.pool,
                        &job.id,
                        "FAILED",
                        Some("OpenCode daemon is offline"),
                    )
                    .await;
                    settlement::mark_schedule(&self.pool, &self.events, &job, "SKIPPED_OFFLINE")
                        .await;
                    if let Some(task) = &task {
                        let _ = tasks::set_phase_status(
                            &self.pool,
                            &task.id,
                            Some("failed"),
                            Some("BLOCKED"),
                        )
                        .await;
                        let _ = tasks::set_blocked_reason(
                            &self.pool,
                            &task.id,
                            Some("OpenCode daemon is offline"),
                        )
                        .await;
                        let _ = tasks::set_last_run(
                            &self.pool,
                            &task.id,
                            Some(&run.id),
                            Some("FAILED"),
                        )
                        .await;
                        self.events.publish(OpenDeskWsEvent::TaskUpdated {
                            task_id: task.id.clone(),
                            workspace_id: task.workspace_id.clone(),
                            stage: "BLOCKED".into(),
                            phase_status: Some("failed".into()),
                        });
                    }
                    self.publish_queue(&job.workspace_id).await;
                    return;
                }
            }
        } else {
            None
        };

        let cancel = CancellationToken::new();
        self.running_jobs
            .lock()
            .expect("running jobs mutex")
            .insert(job.id.clone(), cancel.clone());

        let started = Instant::now();
        let secrets = SecretStore::new(&self.paths);
        let request = RunRequest {
            pool: &self.pool,
            secrets: &secrets,
            workspace_id: &job.workspace_id,
            run_id: &run_id,
            task_id: job.task_id.as_deref(),
            session_id: job.session_id.as_deref(),
            workspace_root: &workspace_path,
            agent: &job.resolved_agent,
            model: &job.resolved_model,
            phase: &phase,
            title: &title,
            prompt: &job.prompt_payload,
            log_path: &log_path,
            cancel,
        };

        let rest_for_compact = rest.clone();
        let mut outcome = match rest {
            Some(rest) => session_runner::execute_run(rest, request, &self.events).await,
            None => match self
                .adapters
                .runtime(&job.resolved_harness, self.daemon.clone())
            {
                Ok(adapter) => {
                    crate::adapters::execution::execute(adapter, request, &self.events).await
                }
                Err(error) => crate::adapters::execution::failed_outcome(
                    request.log_path,
                    format!("harness {} indisponível: {error}", job.resolved_harness),
                ),
            },
        };
        if self
            .lease_expired
            .lock()
            .expect("lease set mutex")
            .remove(&job.id)
        {
            outcome.status = session_runner::RunStatus::Failed;
            outcome.error = Some(
                "lease expirou: o agente ficou 60s sem heartbeat; execução interrompida".into(),
            );
        }
        if !outcome.session_id.is_empty() {
            let _ = runs::set_session(&self.pool, &run_id, Some(&outcome.session_id)).await;
        }

        let reason = format!(
            "{} · fase {}",
            task.as_ref()
                .map(|task| task.title.clone())
                .unwrap_or(title.clone()),
            phase
        );

        run_execution::record_artifacts(
            &self.pool,
            &self.events,
            &job,
            &run_id,
            &reason,
            &workspace_path,
            &output_dir,
            &outcome,
        )
        .await;

        settlement::record_permissions(&self.pool, &run_id, &outcome).await;

        settlement::finish_run(
            &self.pool,
            &run_id,
            &outcome,
            started.elapsed().as_millis() as i64,
        )
        .await;

        let mut fallback_enqueued = false;
        if outcome.status == RunStatus::Success {
            let _ = model_health::record_success(&self.pool, &job.resolved_model).await;
            // Keep the session usable for follow-ups: compact automatically
            // once the context is close to the 128k window.
            if outcome.usage.tokens_input > 100_000
                && !outcome.session_id.is_empty()
                && let Some(rest) = rest_for_compact.as_ref()
            {
                let _ = rest.compact_session(&outcome.session_id).await;
            }
        } else {
            let message = outcome
                .error
                .as_deref()
                .or(outcome.summary.as_deref())
                .unwrap_or("run failed without a message");
            let (code, hint) = settlement::classify_run_error(message);
            let _ = runs::set_error(&self.pool, &run_id, code, message, hint).await;
            if matches!(code, "rate_limited" | "provider_quota") {
                let minutes = if code == "provider_quota" { 60 } else { 5 };
                let _ =
                    model_health::record_cooldown(&self.pool, &job.resolved_model, code, minutes)
                        .await;
                if job.job_kind != "fallback"
                    && let Some(fallback) = self
                        .pick_fallback_model(
                            &job.workspace_id,
                            &workspace_path,
                            &job.resolved_model,
                        )
                        .await
                {
                    let _ = self
                        .enqueue(queue::EnqueueInput {
                            workspace_id: job.workspace_id.clone(),
                            task_id: job.task_id.clone(),
                            schedule_id: job.schedule_id.clone(),
                            session_id: None,
                            concurrency_group: job.concurrency_group.clone(),
                            prompt_payload: job.prompt_payload.clone(),
                            resolved_mode: job.resolved_mode.clone(),
                            resolved_agent: job.resolved_agent.clone(),
                            resolved_model: fallback.clone(),
                            resolved_effort: job.resolved_effort.clone(),
                            resolved_harness: job.resolved_harness.clone(),
                            job_kind: "fallback".into(),
                            scheduled_for: None,
                            output_dir: job.output_dir.clone(),
                            priority: job.priority,
                            max_retries: 1,
                        })
                        .await;
                    if let Some(task) = &task {
                        let _ = tasks::set_phase_status(
                            &self.pool,
                            &task.id,
                            Some("fallback_queued"),
                            Some("TODO"),
                        )
                        .await;
                        self.events.publish(OpenDeskWsEvent::TaskUpdated {
                            task_id: task.id.clone(),
                            workspace_id: task.workspace_id.clone(),
                            stage: "TODO".into(),
                            phase_status: Some("fallback_queued".into()),
                        });
                    }
                    fallback_enqueued = true;
                }
            } else if matches!(code, "model_unavailable" | "auth_failed") {
                let _ = model_health::record_failure(&self.pool, &job.resolved_model, code).await;
            }
        }

        // Learning runs after error classification so lessons include the final
        // code/hint. It is best-effort and never blocks settlement.
        if let Ok(Some(lesson)) = crate::learning::capture_run(&self.pool, &run_id).await {
            tracing::debug!(lesson_id = %lesson.id, "learning lesson captured");
            self.events.publish(OpenDeskWsEvent::LearningUpdated {
                workspace_id: job.workspace_id.clone(),
            });
        }

        settlement::settle_queue(&self.pool, &self.events, &job, &outcome).await;

        if let Some(task) = &task {
            settlement::settle_task(
                &self.pool,
                &self.events,
                task,
                &phase,
                &outcome,
                &run_id,
                fallback_enqueued,
            )
            .await;
        }

        settlement::publish_run_finished(&self.events, &job, &run_id, &outcome);

        let delivery_pool = self.pool.clone();
        let delivery_paths = self.paths.clone();
        let delivery_workspace = job.workspace_id.clone();
        let delivery_task = job.task_id.clone();
        let delivery_schedule = job.schedule_id.clone();
        let delivery_run = run_id.clone();
        let delivery_google = self.google.clone();
        let io_ctx = crate::io_bus::IoContext {
            pool: self.pool.clone(),
            paths: self.paths.clone(),
        };
        let run_success = outcome.status == RunStatus::Success;
        tauri::async_runtime::spawn(async move {
            if let Err(error) = delivery::deliver_run(
                &delivery_pool,
                &delivery_paths,
                &delivery_workspace,
                delivery_task.as_deref(),
                delivery_schedule.as_deref(),
                &delivery_run,
                delivery_google,
            )
            .await
            {
                tracing::warn!(error = %error, "post-run delivery failed");
            }
            if run_success
                && let Err(error) = crate::io_bus::run_sinks(
                    &io_ctx,
                    &delivery_workspace,
                    delivery_task.as_deref(),
                    delivery_schedule.as_deref(),
                    &delivery_run,
                )
                .await
            {
                tracing::warn!(error = %error, "post-run io sinks failed");
            }
        });

        self.running_jobs
            .lock()
            .expect("running jobs mutex")
            .remove(&job.id);
        self.publish_queue(&job.workspace_id).await;

        if let Ok(check) = budget::check(&self.pool).await
            && !check.allowed
        {
            self.pause(check.message.clone()).await;
            self.events.publish(OpenDeskWsEvent::BudgetAlert {
                level: check.level.into(),
                daily_spent: check.status.daily_spent,
                daily_limit: check.status.daily_limit,
                message: check.message,
            });
        }
    }

    async fn pick_fallback_model(
        &self,
        workspace_id: &str,
        workspace_root: &Path,
        current: &str,
    ) -> Option<String> {
        let models = self
            .daemon
            .models(Some(&workspace_root.to_string_lossy()))
            .await
            .ok()?;
        let cooling = model_health::cooling(&self.pool, 24)
            .await
            .unwrap_or_default();
        let current_base = current.split('#').next().unwrap_or(current).to_string();
        let key = |model: &crate::opencode_client::rest_client::ModelDto| {
            let provider = model.provider_id.clone().unwrap_or_default();
            let id = model.model_id.clone().unwrap_or_else(|| model.id.clone());
            format!("{provider}/{id}")
        };
        let router_models = router_db::models(&self.pool, workspace_id)
            .await
            .unwrap_or_default();
        let mut preferred = Vec::new();
        if let Some(model) = router_models.default_model.as_deref() {
            preferred.push(model);
        }
        if let Some(model) = router_models.manual_model.as_deref() {
            preferred.push(model);
        }
        preferred.extend(["opencode-go/deepseek-v4.1-flash", "opencode-go/glm-5.3"]);
        let mut candidates = models
            .iter()
            .filter(|model| model.enabled.unwrap_or(true))
            .filter(|model| {
                let name = key(model);
                name != current_base && !cooling.contains(&name)
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|model| {
            let name = key(model);
            preferred
                .iter()
                .position(|candidate| *candidate == name)
                .unwrap_or(usize::MAX)
        });
        let model = candidates.first()?;
        let base = key(model);
        if model.variants.iter().any(|variant| variant.id == "low") {
            Some(format!("{base}#low"))
        } else {
            Some(base)
        }
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

impl QueueManager {}

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

fn job_dto(job: queue::QueueJob) -> QueueJobDto {
    QueueJobDto {
        id: job.id,
        task_id: job.task_id,
        title: job.task_title,
        status: job.status,
        priority: job.priority as i32,
        concurrency_group: job.concurrency_group,
        lease_expires_at: job.lease_expires_at,
        last_heartbeat_at: job.last_heartbeat_at,
        resolved_mode: job.resolved_mode,
        resolved_agent: job.resolved_agent,
        resolved_model: job.resolved_model,
        resolved_effort: job.resolved_effort,
        job_kind: job.job_kind,
        output_dir: job.output_dir,
        scheduled_for: job.scheduled_for,
        enqueued_at: job.enqueued_at,
        started_at: job.started_at,
        finished_at: job.finished_at,
        retry_count: job.retry_count,
        max_retries: job.max_retries,
        error_message: job.error_message,
        task_origin: job.task_origin,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_model_handles_provider_and_variant() {
        assert_eq!(
            split_model("opencode-go/glm-5.3#high"),
            (Some("opencode-go".into()), "glm-5.3".into())
        );
        assert_eq!(
            split_model("deepseek/deepseek-flash"),
            (Some("deepseek".into()), "deepseek-flash".into())
        );
        assert_eq!(split_model("plain-model"), (None, "plain-model".into()));
        assert_eq!(
            split_model("provider/model#"),
            (Some("provider".into()), "model".into())
        );
    }

    #[test]
    fn job_dto_preserves_operational_fields() {
        let dto = job_dto(queue::QueueJob {
            id: "job-1".into(),
            workspace_id: "ws-1".into(),
            task_id: Some("task-1".into()),
            schedule_id: None,
            session_id: Some("session-1".into()),
            concurrency_group: Some("web_scraper".into()),
            prompt_payload: "prompt".into(),
            resolved_mode: "build".into(),
            resolved_agent: "build".into(),
            resolved_model: "opencode-go/glm-5.3".into(),
            resolved_effort: "high".into(),
            resolved_harness: "opencode".into(),
            job_kind: "task".into(),
            scheduled_for: None,
            output_dir: Some("/tmp/out".into()),
            priority: 3,
            status: "QUEUED".into(),
            retry_count: 1,
            max_retries: 2,
            error_message: Some("retry me".into()),
            enqueued_at: "2026-11-11T00:00:00Z".into(),
            started_at: None,
            finished_at: None,
            lease_expires_at: Some("2026-11-11T00:01:00Z".into()),
            last_heartbeat_at: Some("2026-11-11T00:00:30Z".into()),
            task_title: Some("Relatório".into()),
            task_origin: Some("MANUAL".into()),
        });
        assert_eq!(dto.id, "job-1");
        assert_eq!(dto.title.as_deref(), Some("Relatório"));
        assert_eq!(dto.priority, 3);
        assert_eq!(dto.concurrency_group.as_deref(), Some("web_scraper"));
        assert_eq!(dto.retry_count, 1);
        assert_eq!(dto.error_message.as_deref(), Some("retry me"));
    }

    #[test]
    fn concurrency_is_clamped_to_a_safe_range() {
        assert_eq!(clamp_concurrency(0), 1);
        assert_eq!(clamp_concurrency(1), 1);
        assert_eq!(clamp_concurrency(3), 3);
        assert_eq!(clamp_concurrency(4), 4);
        assert_eq!(clamp_concurrency(99), 4);
    }
}
