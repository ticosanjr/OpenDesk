use crate::{
    db::{
        artifacts::{self, CreateArtifactInput},
        budget, model_health, queue, runs, tasks, workspaces,
    },
    dsh_router::{self, PackContext},
    error::{AppError, AppResult},
    events::{ArtifactProvenanceDto, EventBus, OpenDeskWsEvent, QueueJobDto},
    integrations::{delivery, google_oauth::GoogleOAuth},
    opencode_client::{
        daemon_manager::DaemonManager,
        session_runner::{self, RunRequest, RunStatus},
    },
    paths::AppPaths,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
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
use uuid::Uuid;

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
    google: Arc<GoogleOAuth>,
    paused: AtomicBool,
    pause_reason: Mutex<Option<String>>,
    concurrency: AtomicUsize,
    active: AtomicUsize,
    notify: Notify,
    shutdown: CancellationToken,
    running_jobs: Mutex<HashMap<String, CancellationToken>>,
}

impl QueueManager {
    pub fn new(
        pool: SqlitePool,
        paths: AppPaths,
        events: EventBus,
        daemon: Arc<DaemonManager>,
        google: Arc<GoogleOAuth>,
        concurrency: usize,
    ) -> Arc<Self> {
        Arc::new(Self {
            pool,
            paths,
            events,
            daemon,
            google,
            paused: AtomicBool::new(false),
            pause_reason: Mutex::new(None),
            concurrency: AtomicUsize::new(clamp_concurrency(concurrency)),
            active: AtomicUsize::new(0),
            notify: Notify::new(),
            shutdown: CancellationToken::new(),
            running_jobs: Mutex::new(HashMap::new()),
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

    async fn recover(&self) -> AppResult<()> {
        let runs = runs::recover_running(&self.pool).await?;
        let jobs = queue::recover_running(&self.pool).await?;
        if runs > 0 || jobs > 0 {
            tracing::warn!(runs, jobs, "recovered interrupted jobs after restart");
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
                self.publish_queue(&job.workspace_id).await;
                return;
            }
        };

        let task = match &job.task_id {
            Some(task_id) => tasks::get(&self.pool, task_id).await.ok(),
            None => None,
        };

        let run_id = Uuid::now_v7().to_string();
        let log_path = self.paths.run_logs_dir.join(format!("{run_id}.jsonl"));
        let phase = if job.job_kind == "audit" {
            "audit"
        } else {
            job.resolved_mode.as_str()
        };
        let (provider_id, model_id) = split_model(&job.resolved_model);
        let title = task
            .as_ref()
            .map(|task| task.title.clone())
            .unwrap_or_else(|| {
                job.task_title
                    .clone()
                    .unwrap_or_else(|| "Scheduled task".into())
            });
        let workspace_path = PathBuf::from(&workspace.root_path);
        let output_dir = job
            .output_dir
            .clone()
            .map(PathBuf::from)
            .or_else(|| {
                task.as_ref()
                    .and_then(|task| task.output_dir_path.clone())
                    .map(PathBuf::from)
            })
            .unwrap_or_else(|| {
                workspace_path
                    .join("output")
                    .join(chrono::Utc::now().format("%Y-%m").to_string())
                    .join(crate::dsh_router::slugify(&title))
            });
        let output_dir_string = output_dir.to_string_lossy().to_string();

        if let Err(error) = std::fs::create_dir_all(&output_dir) {
            tracing::warn!(path = %output_dir.display(), error = %error, "could not create output directory");
        }

        let run = match runs::create(
            &self.pool,
            runs::CreateRunInput {
                id: run_id.clone(),
                queue_job_id: Some(job.id.clone()),
                workspace_id: job.workspace_id.clone(),
                task_id: job.task_id.clone(),
                parent_run_id: None,
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
                self.publish_queue(&job.workspace_id).await;
                return;
            }
        };

        let Some(rest) = self.daemon.client().await else {
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
            if let Some(task) = &task {
                let _ = tasks::set_phase_status(&self.pool, &task.id, Some("failed"), None).await;
                self.events.publish(OpenDeskWsEvent::TaskUpdated {
                    task_id: task.id.clone(),
                    workspace_id: task.workspace_id.clone(),
                    stage: task.column_stage.clone(),
                    phase_status: Some("failed".into()),
                });
            }
            self.publish_queue(&job.workspace_id).await;
            return;
        };

        let cancel = CancellationToken::new();
        self.running_jobs
            .lock()
            .expect("running jobs mutex")
            .insert(job.id.clone(), cancel.clone());

        let started = Instant::now();
        let request = RunRequest {
            run_id: &run_id,
            task_id: job.task_id.as_deref(),
            workspace_root: &workspace_path,
            agent: &job.resolved_agent,
            model: &job.resolved_model,
            phase,
            title: &title,
            prompt: &job.prompt_payload,
            log_path: &log_path,
            cancel,
        };

        let outcome = session_runner::execute_run(rest.clone(), request, &self.events).await;

        let reason = format!(
            "{} · fase {}",
            task.as_ref()
                .map(|task| task.title.clone())
                .unwrap_or(title.clone()),
            phase
        );

        let mut seen_files = HashSet::new();
        for diff in &outcome.diffs {
            seen_files.insert(diff.file.clone());
            let action = match diff.status.as_str() {
                "added" => "CREATED",
                "deleted" => "DELETED",
                _ => "MODIFIED",
            };
            self.record_artifact(
                &job.workspace_id,
                job.task_id.as_deref(),
                &run_id,
                &job.resolved_agent,
                &reason,
                &workspace_path,
                &diff.file,
                action,
                Some(&diff.patch),
            )
            .await;
        }

        for write in &outcome.writes {
            if seen_files.contains(&write.path) {
                continue;
            }
            self.record_artifact(
                &job.workspace_id,
                job.task_id.as_deref(),
                &run_id,
                &job.resolved_agent,
                &reason,
                &workspace_path,
                &write.path,
                &write.action,
                write.diff_patch.as_deref(),
            )
            .await;
        }

        for permission in &outcome.permissions {
            let _ = sqlx::query(
                "INSERT INTO permission_audit (
                    id, run_id, session_id, request_id, permission_action, resource, decision
                 ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(Uuid::now_v7().to_string())
            .bind(&run_id)
            .bind(&outcome.session_id)
            .bind(&permission.request_id)
            .bind(&permission.action)
            .bind(permission.resources.join(","))
            .bind(&permission.decision)
            .execute(&self.pool)
            .await;
        }

        let mut fallback_enqueued = false;
        let _ = runs::finish(
            &self.pool,
            &run_id,
            outcome.status.as_str(),
            &outcome.usage,
            started.elapsed().as_millis() as i64,
            outcome.summary.as_deref(),
            outcome.log_tail.as_deref(),
        )
        .await;

        if outcome.status != RunStatus::Success {
            let message = outcome
                .error
                .as_deref()
                .or(outcome.summary.as_deref())
                .unwrap_or("run failed without a message");
            let (code, hint) = classify_run_error(message);
            let _ = runs::set_error(&self.pool, &run_id, code, message, hint).await;
            if matches!(code, "model_unavailable" | "auth_failed") {
                let _ = model_health::record_failure(&self.pool, &job.resolved_model, code).await;
                if job.job_kind != "fallback"
                    && let Some(fallback) = self
                        .pick_fallback_model(&workspace_path, &job.resolved_model)
                        .await
                {
                    let _ = self
                        .enqueue(queue::EnqueueInput {
                            workspace_id: job.workspace_id.clone(),
                            task_id: job.task_id.clone(),
                            schedule_id: job.schedule_id.clone(),
                            prompt_payload: job.prompt_payload.clone(),
                            resolved_mode: job.resolved_mode.clone(),
                            resolved_agent: job.resolved_agent.clone(),
                            resolved_model: fallback.clone(),
                            resolved_effort: job.resolved_effort.clone(),
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
                            None,
                        )
                        .await;
                        self.events.publish(OpenDeskWsEvent::TaskUpdated {
                            task_id: task.id.clone(),
                            workspace_id: task.workspace_id.clone(),
                            stage: task.column_stage.clone(),
                            phase_status: Some("fallback_queued".into()),
                        });
                    }
                    fallback_enqueued = true;
                }
            }
        } else {
            let _ = model_health::record_success(&self.pool, &job.resolved_model).await;
        }

        let queue_status = match outcome.status {
            RunStatus::Success => "COMPLETED",
            RunStatus::Failed => "FAILED",
            RunStatus::Cancelled => "CANCELLED",
        };
        let _ = queue::finish(&self.pool, &job.id, queue_status, outcome.error.as_deref()).await;

        if let Some(task) = &task
            && !fallback_enqueued
        {
            let stage: String;
            let phase_status: String;

            if phase == "plan" && outcome.status == RunStatus::Success {
                tasks::set_plan(
                    &self.pool,
                    &task.id,
                    outcome
                        .summary
                        .as_deref()
                        .unwrap_or("Plano concluído sem texto retornado pelo modelo."),
                )
                .await
                .ok();
                if task.execution_mode == "PLAN_ONLY" {
                    let _ = tasks::set_phase_status(
                        &self.pool,
                        &task.id,
                        Some("plan_complete"),
                        Some("PLAN"),
                    )
                    .await;
                    phase_status = "plan_complete".into();
                } else {
                    phase_status = "awaiting_approval".into();
                }
                stage = "PLAN".into();
            } else if phase == "build" && outcome.status == RunStatus::Success {
                let _ = tasks::set_phase_status(
                    &self.pool,
                    &task.id,
                    Some("build_complete"),
                    Some("REVIEW"),
                )
                .await;
                stage = "REVIEW".into();
                phase_status = "build_complete".into();
            } else if phase == "audit" && outcome.status == RunStatus::Success {
                let approved = outcome
                    .summary
                    .as_deref()
                    .map(|summary| summary.to_uppercase().contains("APROVADO"))
                    .unwrap_or(false);
                let _ = tasks::set_phase_status(
                    &self.pool,
                    &task.id,
                    Some(if approved {
                        "audit_approved"
                    } else {
                        "audit_failed"
                    }),
                    Some(if approved { "DONE" } else { "REVIEW" }),
                )
                .await;
                stage = if approved { "DONE" } else { "REVIEW" }.into();
                phase_status = if approved {
                    "audit_approved".into()
                } else {
                    "audit_failed".into()
                };
            } else {
                let _ = tasks::set_phase_status(&self.pool, &task.id, Some("failed"), None).await;
                stage = task.column_stage.clone();
                phase_status = "failed".into();
            }

            self.events.publish(OpenDeskWsEvent::TaskUpdated {
                task_id: task.id.clone(),
                workspace_id: task.workspace_id.clone(),
                stage: stage.clone(),
                phase_status: Some(phase_status.clone()),
            });

            if phase == "build"
                && outcome.status == RunStatus::Success
                && job.job_kind != "audit"
                && workspace.default_router_policy == "BEST_QUALITY"
            {
                let tags = task.tags();
                let audit_prompt = dsh_router::pack(&PackContext {
                    workspace_root: &workspace_path,
                    output_directory: &output_dir_string,
                    task_title: &task.title,
                    task_description: &task.description,
                    tags: &tags,
                    priority: task.priority,
                    phase: "audit",
                    plan: task.plan_markdown.as_deref(),
                    agent: "explore",
                });
                if let Ok(audit_job) = self
                    .enqueue(queue::EnqueueInput {
                        workspace_id: task.workspace_id.clone(),
                        task_id: Some(task.id.clone()),
                        schedule_id: None,
                        prompt_payload: audit_prompt,
                        resolved_mode: "build".into(),
                        resolved_agent: "explore".into(),
                        resolved_model: job.resolved_model.clone(),
                        resolved_effort: "high".into(),
                        job_kind: "audit".into(),
                        scheduled_for: None,
                        output_dir: Some(output_dir_string.clone()),
                        priority: task.priority,
                        max_retries: 1,
                    })
                    .await
                {
                    let _ = tasks::set_phase_status(
                        &self.pool,
                        &task.id,
                        Some("audit_queued"),
                        Some("REVIEW"),
                    )
                    .await;
                    self.events.publish(OpenDeskWsEvent::TaskUpdated {
                        task_id: task.id.clone(),
                        workspace_id: task.workspace_id.clone(),
                        stage: "REVIEW".into(),
                        phase_status: Some("audit_queued".into()),
                    });
                    let _ = audit_job;
                }
            }
        }

        self.events.publish(OpenDeskWsEvent::RunFinished {
            run_id: run_id.clone(),
            task_id: job.task_id.clone(),
            status: outcome.status.as_str().to_string(),
            estimated_cost_usd: outcome.usage.estimated_cost_usd,
            tokens_input: outcome.usage.tokens_input,
            tokens_output: outcome.usage.tokens_output,
        });

        let delivery_pool = self.pool.clone();
        let delivery_paths = self.paths.clone();
        let delivery_workspace = job.workspace_id.clone();
        let delivery_task = job.task_id.clone();
        let delivery_schedule = job.schedule_id.clone();
        let delivery_run = run_id.clone();
        let delivery_google = self.google.clone();
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

    #[allow(clippy::too_many_arguments)]
    async fn record_artifact(
        &self,
        workspace_id: &str,
        task_id: Option<&str>,
        run_id: &str,
        agent: &str,
        reason: &str,
        workspace_root: &Path,
        file_path: &str,
        action: &str,
        diff_patch: Option<&str>,
    ) {
        if artifacts::exists(&self.pool, run_id, file_path)
            .await
            .unwrap_or(false)
        {
            return;
        }

        let absolute = if Path::new(file_path).is_absolute() {
            PathBuf::from(file_path)
        } else {
            workspace_root.join(file_path)
        };
        let relative = absolute
            .strip_prefix(workspace_root)
            .map(|path| path.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| file_path.to_string());
        let file_type = absolute
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("")
            .to_string();
        let (size, hash) = file_fingerprint(&absolute);

        match artifacts::create(
            &self.pool,
            CreateArtifactInput {
                workspace_id: workspace_id.to_string(),
                task_id: task_id.map(str::to_string),
                run_id: run_id.to_string(),
                file_path: relative,
                absolute_path: absolute.to_string_lossy().to_string(),
                file_type,
                file_size_bytes: size,
                content_sha256: hash,
                previous_sha256: None,
                action_type: action.to_string(),
                created_by_agent: agent.to_string(),
                creation_reason: reason.to_string(),
                diff_patch: diff_patch.map(str::to_string),
            },
        )
        .await
        {
            Ok(artifact) => self.events.publish(OpenDeskWsEvent::ArtifactCreated {
                artifact: artifact_dto(&artifact),
            }),
            Err(error) => tracing::warn!(error = %error, "could not record artifact"),
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

impl QueueManager {
    async fn pick_fallback_model(&self, workspace_root: &Path, current: &str) -> Option<String> {
        let models = self
            .daemon
            .models(Some(&workspace_root.to_string_lossy()))
            .await
            .ok()?;
        let cooling = model_health::cooling(&self.pool, 24)
            .await
            .unwrap_or_default();
        let current_base = current.split('#').next().unwrap_or(current);
        let key = |model: &crate::opencode_client::rest_client::ModelDto| {
            let provider = model.provider_id.clone().unwrap_or_default();
            let id = model.model_id.clone().unwrap_or_else(|| model.id.clone());
            format!("{provider}/{id}")
        };
        let preferred = [
            "deepseek/deepseek-flash",
            "opencode-go/glm-5.3",
            "opencode-go/deepseek-v4.1-flash",
        ];
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
}

fn classify_run_error(message: &str) -> (&'static str, &'static str) {
    let lower = message.to_lowercase();
    if lower.contains("daemon is offline") || lower.contains("connection refused") {
        (
            "engine_offline",
            "O motor OpenCode não está acessível. Reinicie o motor em Configurações → OpenCode Engine.",
        )
    } else if lower.contains("401") || lower.contains("unauthorized") || lower.contains("password")
    {
        (
            "auth_failed",
            "O OpenCode rejeitou a autenticação. Reinicie o motor para renovar a senha do daemon.",
        )
    } else if lower.contains("model") && (lower.contains("not found") || lower.contains("unknown"))
    {
        (
            "model_not_found",
            "O modelo configurado não existe ou não está disponível para este workspace. Edite a tarefa e escolha outro modelo.",
        )
    } else if lower.contains("agent") && lower.contains("not found") {
        (
            "agent_not_found",
            "O agente configurado não existe no workspace. Edite a tarefa e escolha outro agente.",
        )
    } else if lower.contains("timed out") || lower.contains("timeout") {
        (
            "timeout",
            "A execução excedeu o tempo limite. Tente novamente ou reduza o escopo da tarefa.",
        )
    } else if lower.contains("cancel") {
        (
            "cancelled",
            "A execução foi cancelada. Use Tentar de novo para enfileirar outra execução.",
        )
    } else if lower.contains("budget") {
        (
            "budget_blocked",
            "O budget guard bloqueou a execução. Ajuste o limite em Roteador & Custos ou aguarde o próximo período.",
        )
    } else if lower.contains("no such file") || lower.contains("enoent") {
        (
            "binary_missing",
            "O binário do OpenCode não foi encontrado. Configure o caminho em Configurações → OpenCode Engine.",
        )
    } else {
        (
            "run_failed",
            "Abra o log completo e o diagnóstico da tarefa. Se o erro citar modelo/agente, edite a tarefa e tente novamente.",
        )
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

fn file_fingerprint(path: &Path) -> (i64, Option<String>) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return (0, None);
    };
    if !metadata.is_file() {
        return (0, None);
    }
    let hash = std::fs::read(path).ok().map(|bytes| {
        let digest = Sha256::digest(&bytes);
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    });
    (metadata.len() as i64, hash)
}

fn job_dto(job: queue::QueueJob) -> QueueJobDto {
    QueueJobDto {
        id: job.id,
        task_id: job.task_id,
        title: job.task_title,
        status: job.status,
        priority: job.priority as i32,
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
    }
}

fn artifact_dto(artifact: &artifacts::ArtifactProvenance) -> ArtifactProvenanceDto {
    ArtifactProvenanceDto {
        id: artifact.id.clone(),
        workspace_id: artifact.workspace_id.clone(),
        task_id: artifact.task_id.clone(),
        run_id: artifact.run_id.clone(),
        file_path: artifact.file_path.clone(),
        absolute_path: artifact.absolute_path.clone(),
        file_type: artifact.file_type.clone(),
        action_type: artifact.action_type.clone(),
        created_by_agent: artifact.created_by_agent.clone(),
        creation_reason: artifact.creation_reason.clone(),
        diff_patch: artifact.diff_patch.clone(),
        created_at: artifact.created_at.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_common_run_failures() {
        assert_eq!(
            classify_run_error("OpenCode daemon is offline").0,
            "engine_offline"
        );
        assert_eq!(classify_run_error("HTTP 401 Unauthorized").0, "auth_failed");
        assert_eq!(
            classify_run_error("model not found: x").0,
            "model_not_found"
        );
        assert_eq!(
            classify_run_error("agent not found: x").0,
            "agent_not_found"
        );
        assert_eq!(classify_run_error("request timed out").0, "timeout");
        assert_eq!(classify_run_error("cancelled by user").0, "cancelled");
        assert_eq!(classify_run_error("budget hard stop").0, "budget_blocked");
        assert_eq!(
            classify_run_error("No such file or directory").0,
            "binary_missing"
        );
        assert_eq!(classify_run_error("something else").0, "run_failed");
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
