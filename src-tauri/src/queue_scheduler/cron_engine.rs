pub use crate::recurrence::next_occurrence;
use crate::{
    config_sync::{commands, paths::StudioPaths},
    db::{
        missed_runs,
        queue::{EnqueueInput, QueueJob},
        router as router_db,
        schedules::{self, Schedule},
        settings, tasks, workspaces,
    },
    dsh_router::{self, PackContext, RouterInput},
    error::{AppError, AppResult},
    events::{EventBus, OpenDeskWsEvent},
    opencode_client::daemon_manager::DaemonManager,
    paths::AppPaths,
    queue_scheduler::QueueManager,
};
use chrono::{DateTime, Datelike, Timelike, Utc};
use chrono_tz::Tz;
use serde::Serialize;
use sqlx::SqlitePool;
use std::{
    path::{Component, Path, PathBuf},
    sync::{Arc, OnceLock, Weak},
    time::Duration,
};
use tokio_util::sync::CancellationToken;

pub struct CronEngine {
    pool: SqlitePool,
    #[allow(dead_code)]
    paths: AppPaths,
    events: EventBus,
    queue: Arc<QueueManager>,
    daemon: Arc<DaemonManager>,
    shutdown: CancellationToken,
    /// Set after AppState is wrapped in an Arc. Weak avoids an ownership cycle
    /// while still letting a CRON routine start the recipe engine.
    app: OnceLock<Weak<crate::app_state::AppState>>,
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
    ) -> Arc<Self> {
        Arc::new(Self {
            pool,
            paths,
            events,
            queue,
            daemon,
            shutdown: CancellationToken::new(),
            app: OnceLock::new(),
        })
    }

    pub fn spawn(self: &Arc<Self>) {
        let engine = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = engine.shutdown.cancelled() => break,
                    _ = tokio::time::sleep(Duration::from_secs(15)) => {}
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

    pub fn attach_state(&self, state: &Arc<crate::app_state::AppState>) {
        let _ = self.app.set(Arc::downgrade(state));
    }

    pub async fn tick_once(&self) -> AppResult<CronTickResult> {
        let schedules = schedules::active(&self.pool).await?;
        let now = Utc::now();
        let mut enqueued = 0;

        let engine_online = self.daemon.status().await.online;

        for schedule in &schedules {
            // Non-cron routines are driven by the IO bus / Signal bridge, not
            // by the cron tick.
            if schedule.trigger_type != "CRON" {
                continue;
            }
            if schedule.target_kind == "RECIPE" {
                if schedule.next_run_at.is_none() {
                    if let Ok(next) =
                        next_occurrence(&schedule.cron_expression, &schedule.timezone, now)
                    {
                        let _ = schedules::set_next_run(
                            &self.pool,
                            &schedule.id,
                            Some(&next.to_rfc3339()),
                        )
                        .await;
                    }
                    continue;
                }
                let due = schedule
                    .next_run_at
                    .as_deref()
                    .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                    .map(|value| value.with_timezone(&Utc) <= now)
                    .unwrap_or(false);
                if due {
                    let scheduled_for = schedule
                        .next_run_at
                        .clone()
                        .unwrap_or_else(|| now.to_rfc3339());
                    let scheduled_dt = DateTime::parse_from_rfc3339(&scheduled_for)
                        .map(|value| value.with_timezone(&Utc))
                        .unwrap_or(now);
                    let overdue = now.signed_duration_since(scheduled_dt);
                    if !engine_online || overdue.num_seconds() > 90 {
                        let reason = if overdue.num_seconds() > 90 {
                            "MISFIRE"
                        } else if !engine_online {
                            "OFFLINE"
                        } else {
                            "MISFIRE"
                        };
                        self.record_missed_occurrences(schedule, scheduled_dt, now, reason)
                            .await;
                        let _ = schedules::mark_run(
                            &self.pool,
                            &schedule.id,
                            if overdue.num_seconds() > 90 || engine_online {
                                "SKIPPED_MISFIRE"
                            } else {
                                "SKIPPED_OFFLINE"
                            },
                            &now.to_rfc3339(),
                        )
                        .await;
                    } else {
                        match self.run_recipe_schedule(schedule, &scheduled_for).await {
                            Ok(_) => {
                                enqueued += 1;
                                let _ = schedules::mark_run(
                                    &self.pool,
                                    &schedule.id,
                                    "QUEUED",
                                    &now.to_rfc3339(),
                                )
                                .await;
                            }
                            Err(error) => {
                                tracing::warn!(schedule = %schedule.id, error = %error, "recipe routine failed");
                                let _ = schedules::mark_run(
                                    &self.pool,
                                    &schedule.id,
                                    "FAILED",
                                    &now.to_rfc3339(),
                                )
                                .await;
                            }
                        }
                    }
                    if schedule.one_time {
                        let _ = schedules::set_active(&self.pool, &schedule.id, false).await;
                        let _ = schedules::set_next_run(&self.pool, &schedule.id, None).await;
                    } else if let Ok(next) =
                        next_occurrence(&schedule.cron_expression, &schedule.timezone, now)
                    {
                        let _ = schedules::set_next_run(
                            &self.pool,
                            &schedule.id,
                            Some(&next.to_rfc3339()),
                        )
                        .await;
                    }
                    self.events.publish(OpenDeskWsEvent::ScheduleUpdated {
                        schedule_id: schedule.id.clone(),
                        workspace_id: schedule.workspace_id.clone(),
                    });
                }
                continue;
            }
            if let Some(recurrence) = crate::queue_scheduler::recurrence::parse_recurrence(
                schedule.recurrence_json.as_deref(),
            ) && recurrence.is_expired(now, &schedule.timezone)
            {
                let _ = schedules::set_active(&self.pool, &schedule.id, false).await;
                let _ = schedules::set_next_run(&self.pool, &schedule.id, None).await;
                self.events.publish(OpenDeskWsEvent::ScheduleUpdated {
                    schedule_id: schedule.id.clone(),
                    workspace_id: schedule.workspace_id.clone(),
                });
                continue;
            }
            let next = match next_occurrence(&schedule.cron_expression, &schedule.timezone, now) {
                Ok(next) => next,
                Err(error) => {
                    tracing::warn!(schedule = %schedule.id, error = %error, "invalid schedule");
                    let _ = schedules::mark_run(
                        &self.pool,
                        &schedule.id,
                        "INVALID_CRON",
                        &now.to_rfc3339(),
                    )
                    .await;
                    continue;
                }
            };

            let due = schedule
                .next_run_at
                .as_deref()
                .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.with_timezone(&Utc) <= now)
                .unwrap_or(false);

            if due {
                let overdue = schedule
                    .next_run_at
                    .as_deref()
                    .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                    .map(|value| now.signed_duration_since(value.with_timezone(&Utc)))
                    .unwrap_or_default();
                if !engine_online || overdue.num_seconds() > 90 {
                    let scheduled_dt = schedule
                        .next_run_at
                        .as_deref()
                        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
                        .map(|value| value.with_timezone(&Utc))
                        .unwrap_or(now);
                    let reason = if overdue.num_seconds() > 90 {
                        "MISFIRE"
                    } else if !engine_online {
                        "OFFLINE"
                    } else {
                        "MISFIRE"
                    };
                    self.record_missed_occurrences(schedule, scheduled_dt, now, reason)
                        .await;
                    let _ = schedules::mark_run(
                        &self.pool,
                        &schedule.id,
                        if overdue.num_seconds() > 90 || engine_online {
                            "SKIPPED_MISFIRE"
                        } else {
                            "SKIPPED_OFFLINE"
                        },
                        &now.to_rfc3339(),
                    )
                    .await;
                    if schedule.one_time {
                        let _ = schedules::set_active(&self.pool, &schedule.id, false).await;
                        let _ = schedules::set_next_run(&self.pool, &schedule.id, None).await;
                    } else {
                        let _ = schedules::set_next_run(
                            &self.pool,
                            &schedule.id,
                            Some(&next.to_rfc3339()),
                        )
                        .await;
                    }
                    self.events.publish(OpenDeskWsEvent::ScheduleUpdated {
                        schedule_id: schedule.id.clone(),
                        workspace_id: schedule.workspace_id.clone(),
                    });
                    continue;
                }
                let scheduled_for = schedule
                    .next_run_at
                    .clone()
                    .unwrap_or_else(|| now.to_rfc3339());
                match self.enqueue_schedule(schedule, &scheduled_for, now).await {
                    Ok(Some(_)) => {
                        enqueued += 1;
                        let _ = schedules::mark_run(
                            &self.pool,
                            &schedule.id,
                            "QUEUED",
                            &now.to_rfc3339(),
                        )
                        .await;
                        if schedule.one_time {
                            let _ = schedules::set_active(&self.pool, &schedule.id, false).await;
                            let _ = schedules::set_next_run(&self.pool, &schedule.id, None).await;
                        }
                    }
                    Ok(None) => {
                        let _ = schedules::mark_run(
                            &self.pool,
                            &schedule.id,
                            "SKIPPED_DUPLICATE",
                            &now.to_rfc3339(),
                        )
                        .await;
                    }
                    Err(error) => {
                        tracing::warn!(schedule = %schedule.id, error = %error, "could not enqueue schedule");
                        let _ = schedules::mark_run(
                            &self.pool,
                            &schedule.id,
                            "FAILED",
                            &now.to_rfc3339(),
                        )
                        .await;
                    }
                }
                if !schedule.one_time {
                    let _ =
                        schedules::set_next_run(&self.pool, &schedule.id, Some(&next.to_rfc3339()))
                            .await;
                }
                self.events.publish(OpenDeskWsEvent::ScheduleUpdated {
                    schedule_id: schedule.id.clone(),
                    workspace_id: schedule.workspace_id.clone(),
                });
            } else if schedule.next_run_at.is_none() {
                let _ = schedules::set_next_run(&self.pool, &schedule.id, Some(&next.to_rfc3339()))
                    .await;
            }
        }

        enqueued += self.reconcile_tasks().await;

        Ok(CronTickResult {
            checked: schedules.len(),
            enqueued,
        })
    }

    /// Auto-start READY tasks that are not held, not awaiting human approval
    /// and not managed by a recipe. Called on every cron tick so tasks created
    /// while the engine was offline start as soon as it comes back.
    async fn reconcile_tasks(&self) -> usize {
        if !settings::bool_flag(&self.pool, "tasks_autostart", true).await {
            return 0;
        }
        if self.queue.is_paused() || !self.daemon.status().await.online {
            return 0;
        }
        let Some(state) = self.app.get().and_then(Weak::upgrade) else {
            return 0;
        };
        let workspaces = workspaces::list(&self.pool).await.unwrap_or_default();
        let mut started = 0;
        for workspace in workspaces {
            if started >= 3 {
                break;
            }
            let tasks = tasks::list(&self.pool, &workspace.id)
                .await
                .unwrap_or_default();
            for task in tasks {
                if started >= 3 {
                    break;
                }
                if task.workflow_state != "READY" || task.origin == "RECIPE" {
                    continue;
                }
                match crate::api::tasks::dispatch_task_internal(
                    &state,
                    &task.id,
                    crate::api::tasks::DispatchBody::default(),
                    None,
                )
                .await
                {
                    Ok(_) => started += 1,
                    Err(error) => {
                        tracing::debug!(task = %task.id, error = %error, "autostart postponed");
                    }
                }
            }
        }
        started
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

    async fn run_recipe_schedule(
        &self,
        schedule: &Schedule,
        scheduled_for: &str,
    ) -> AppResult<serde_json::Value> {
        let recipe_id = schedule
            .recipe_id
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("rotina sem receita vinculada".into()))?;
        let state =
            self.app.get().and_then(Weak::upgrade).ok_or_else(|| {
                AppError::Internal("app state indisponível para a receita".into())
            })?;
        let yaml =
            crate::recipes::store::yaml_for(&state.db, &schedule.workspace_id, recipe_id).await?;
        let recipe = crate::recipes::parser::parse(&yaml)?;
        let trigger = serde_json::json!({
            "routine_id": schedule.id,
            "routine_name": schedule.name,
            "scheduled_for": scheduled_for,
        });
        let run = crate::recipes::engine::start(
            crate::app_state::recipe_context(&state),
            &schedule.workspace_id,
            recipe,
            trigger,
            false,
        )
        .await?;
        Ok(serde_json::json!({
            "run_id": run.id,
            "recipe_id": recipe_id,
        }))
    }

    /// Enqueue a prompt routine immediately without touching its next run.
    /// Record every missed occurrence between `from` and `now` (bounded) and
    /// apply the user's missed-run policy.
    async fn record_missed_occurrences(
        &self,
        schedule: &Schedule,
        from: DateTime<Utc>,
        now: DateTime<Utc>,
        reason: &str,
    ) -> usize {
        let mut cursor = from;
        let mut count = 0;
        while cursor <= now && count < 5 {
            match missed_runs::create_or_ignore(
                &self.pool,
                &schedule.workspace_id,
                &schedule.id,
                &cursor.to_rfc3339(),
                reason,
            )
            .await
            {
                Ok(missed) => {
                    if missed.status == "PENDING" {
                        self.apply_missed_policy(&missed).await;
                    }
                    count += 1;
                }
                Err(error) => {
                    tracing::warn!(schedule = %schedule.id, error = %error, "could not record missed run");
                    break;
                }
            }
            if schedule.one_time {
                break;
            }
            let Ok(next) = next_occurrence(&schedule.cron_expression, &schedule.timezone, cursor)
            else {
                break;
            };
            if next <= cursor {
                break;
            }
            cursor = next;
        }
        count
    }

    async fn apply_missed_policy(&self, missed: &missed_runs::MissedRun) {
        let policy = settings::get(&self.pool, "missed_run_policy")
            .await
            .ok()
            .flatten()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| "ASK".to_string())
            .to_uppercase();
        match policy.as_str() {
            "RUN" => {
                if !self.daemon.status().await.online || self.queue.is_paused() {
                    return;
                }
                match self.run_missed(missed).await {
                    Ok(run_id) => {
                        let _ = missed_runs::set_status(
                            &self.pool,
                            &missed.id,
                            "QUEUED",
                            run_id.as_deref(),
                            None,
                        )
                        .await;
                    }
                    Err(error) => {
                        let _ = missed_runs::set_status(
                            &self.pool,
                            &missed.id,
                            "FAILED",
                            None,
                            Some(&format!("{error:#}")),
                        )
                        .await;
                    }
                }
            }
            "SKIP" => {
                let _ =
                    missed_runs::set_status(&self.pool, &missed.id, "IGNORED", None, None).await;
            }
            _ => {}
        }
    }

    /// Execute one missed occurrence through the same path as "Run now".
    pub async fn run_missed(&self, missed: &missed_runs::MissedRun) -> AppResult<Option<String>> {
        let schedule = schedules::get(&self.pool, &missed.schedule_id).await?;
        let now = Utc::now();
        if schedule.target_kind == "RECIPE" {
            let value = self
                .run_recipe_schedule(&schedule, &missed.scheduled_for)
                .await?;
            Ok(value
                .get("run_id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string))
        } else {
            let job = self
                .enqueue_schedule(&schedule, &missed.scheduled_for, now)
                .await?;
            Ok(job.map(|job| job.id))
        }
    }

    pub async fn enqueue_now(&self, schedule: &Schedule) -> AppResult<Option<QueueJob>> {
        self.enqueue_schedule(schedule, &Utc::now().to_rfc3339(), Utc::now())
            .await
    }

    async fn enqueue_schedule(
        &self,
        schedule: &Schedule,
        scheduled_for: &str,
        now: DateTime<Utc>,
    ) -> AppResult<Option<QueueJob>> {
        let workspace = workspaces::get(&self.pool, &schedule.workspace_id).await?;
        let workspace_root = PathBuf::from(&workspace.root_path);
        let output_directory = render_output_folder(schedule, &workspace_root, now)?;
        std::fs::create_dir_all(&output_directory).map_err(|error| {
            AppError::Internal(format!(
                "could not create schedule output directory: {error}"
            ))
        })?;

        let rules = router_db::list(&self.pool, &schedule.workspace_id).await?;
        let available_models = self
            .daemon
            .models(Some(&workspace.root_path))
            .await
            .unwrap_or_default();
        let router_models = router_db::models(&self.pool, &schedule.workspace_id)
            .await
            .unwrap_or_default();
        let default_harness = crate::adapters::default_harness(&self.pool).await?;
        let input = RouterInput {
            workspace_id: &schedule.workspace_id,
            workspace_root: &workspace_root,
            title: &schedule.name,
            prompt: &schedule.command_or_prompt,
            execution_mode: "AUTO_DSH",
            priority: 2,
            tags: &[],
            manual_agent: schedule.agent_name.as_deref(),
            manual_model: None,
            manual_effort: None,
            manual_harness: None,
            default_harness: Some(&default_harness),
            policy: &schedule.router_policy,
            rules: &rules,
            available_models: &available_models,
            default_model: router_models.default_model.as_deref(),
            manual_default_model: router_models.manual_model.as_deref(),
            approved_plan: None,
            output_directory: Some(&output_directory),
            budget_daily_pct: 0.0,
        };
        let mut decision = dsh_router::classify(&input);
        if let Some(agent) = &schedule.agent_name {
            decision.selected_agent = agent.clone();
        }

        let prompt = self.resolve_prompt(schedule, &workspace_root).await;
        let needs_browser = if crate::browse::needs_browser(&schedule.name, &prompt, &[]) {
            match self.app.get().and_then(Weak::upgrade) {
                Some(app) => crate::browse::ensure_for_task(
                    &app.db,
                    &app.paths,
                    &schedule.workspace_id,
                    &schedule.name,
                    &prompt,
                    &[],
                )
                .await
                .unwrap_or(true),
                None => true,
            }
        } else {
            false
        };
        let personal_context =
            crate::personal_context::render_body(&self.pool, &schedule.workspace_id)
                .await
                .unwrap_or_default();
        let memory_context = crate::db::memory::render_context(
            &self.pool,
            &schedule.workspace_id,
            &decision.selected_agent,
            &format!("{} {}", schedule.name, prompt),
            8,
        )
        .await
        .unwrap_or_default();
        let templates = crate::dsh_router::prompt_templates::PromptTemplates::load(
            &self.pool,
            Some(&schedule.workspace_id),
        )
        .await
        .unwrap_or_default();
        let packed = dsh_router::pack(
            &PackContext {
                workspace_root: &workspace_root,
                output_directory: &decision.output_directory,
                task_id: "",
                task_title: &schedule.name,
                task_description: &prompt,
                tags: &[],
                priority: 2,
                phase: &decision.phase,
                plan: None,
                agent: &decision.selected_agent,
                care_level: "standard",
                needs_browser,
                personal_context: &personal_context,
                structured_memory: &memory_context,
                session_packet: "",
            },
            &templates,
        );

        match self
            .queue
            .enqueue(EnqueueInput {
                workspace_id: schedule.workspace_id.clone(),
                task_id: None,
                schedule_id: Some(schedule.id.clone()),
                session_id: None,
                concurrency_group: Some("light_api".into()),
                prompt_payload: packed,
                resolved_mode: decision.phase,
                resolved_agent: decision.selected_agent,
                resolved_model: decision.selected_model,
                resolved_effort: decision.effort,
                resolved_harness: decision.harness,
                job_kind: "schedule".into(),
                scheduled_for: Some(scheduled_for.to_string()),
                output_dir: Some(decision.output_directory),
                priority: 2,
                max_retries: 1,
            })
            .await
        {
            Ok(job) => Ok(Some(job)),
            Err(AppError::Db(sqlx::Error::Database(database)))
                if database.is_unique_violation() =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    async fn resolve_prompt(&self, schedule: &Schedule, workspace_root: &Path) -> String {
        let trimmed = schedule.command_or_prompt.trim();
        if !trimmed.starts_with('/') {
            return trimmed.to_string();
        }

        let (command, arguments) = trimmed[1..].split_once(' ').unwrap_or((&trimmed[1..], ""));
        let paths = StudioPaths::new(Some(workspace_root.to_path_buf()));
        for scope in ["workspace", "global"] {
            if let Ok(command) = commands::get(&paths, scope, command) {
                return command.template.replace("$ARGUMENTS", arguments);
            }
        }
        trimmed.to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn schedule() -> Schedule {
        Schedule {
            id: "sched-1".into(),
            workspace_id: "ws-1".into(),
            name: "Relatório Diário".into(),
            description: None,
            cron_expression: "0 8 * * 1-5".into(),
            timezone: "America/Sao_Paulo".into(),
            agent_name: None,
            command_or_prompt: "/relatorio".into(),
            trigger_type: "CRON".into(),
            trigger_config_json: "{}".into(),
            target_kind: "PROMPT".into(),
            recipe_id: None,
            router_policy: "COST_BENEFIT".into(),
            output_folder_template: "output/{YYYY}-{MM}/{schedule_slug}".into(),
            notify_email_recipients: None,
            notify_signal: false,
            notify_signal_recipients: None,
            notify_telegram: false,
            notify_telegram_recipients: None,
            notify_only_on_change: false,
            recurrence_json: None,
            one_time: false,
            last_output_hash: None,
            misfire_policy: "SKIP_TO_NEXT".into(),
            safe_readonly_mode: true,
            sync_to_google_docs: false,
            sync_to_google_calendar: false,
            is_active: true,
            last_run_at: None,
            last_run_status: None,
            next_run_at: None,
            created_at: String::new(),
        }
    }

    #[test]
    fn next_run_uses_timezone_and_is_future() {
        let now = DateTime::parse_from_rfc3339("2026-11-06T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let next = next_occurrence("0 8 * * 1-5", "America/Sao_Paulo", now).expect("next");
        assert!(next > now);
        let tz: Tz = "America/Sao_Paulo".parse().expect("tz");
        assert_eq!(next.with_timezone(&tz).format("%H:%M").to_string(), "08:00");
    }

    #[test]
    fn output_template_renders_and_rejects_traversal() {
        let now = DateTime::parse_from_rfc3339("2026-11-06T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let path = render_output_folder(&schedule(), Path::new("/tmp/ws"), now).expect("path");
        assert!(
            path.to_string_lossy()
                .ends_with("output/2026-11/relatorio-diario")
        );

        let mut bad = schedule();
        bad.output_folder_template = "../escape".into();
        assert!(render_output_folder(&bad, Path::new("/tmp/ws"), now).is_err());
    }

    #[test]
    fn output_folder_renders_tokens_and_rejects_escapes() {
        use chrono::TimeZone;
        let mut value = schedule();
        value.output_folder_template = "output/{YYYY}-{MM}/{DD}/{schedule_slug}".into();
        value.name = "Relatório Diário".into();
        let now = chrono::Utc
            .with_ymd_and_hms(2026, 11, 8, 15, 30, 0)
            .unwrap();
        let root = Path::new("/tmp/ws");
        let rendered = render_output_folder(&value, root, now).expect("folder");
        assert_eq!(
            rendered,
            Path::new("/tmp/ws/output/2026-11/08/relatorio-diario")
        );

        value.output_folder_template = "../escape".into();
        assert!(render_output_folder(&value, root, now).is_err());
        value.output_folder_template = "/tmp/outside".into();
        assert!(render_output_folder(&value, root, now).is_err());
        value.output_folder_template = "/tmp/ws/inside".into();
        assert_eq!(
            render_output_folder(&value, root, now).expect("inside"),
            Path::new("/tmp/ws/inside")
        );
    }

    #[tokio::test]
    async fn tick_skips_offline_and_misfired_schedules() {
        let ctx = crate::test_support::context().await;
        let input: crate::db::schedules::CreateScheduleInput =
            serde_json::from_value(serde_json::json!({
                "workspace_id": ctx.workspace_id,
                "name": "Rotina offline",
                "command_or_prompt": "Resuma",
                "cron_expression": "0 8 * * *",
                "timezone": "UTC",
                "trigger_type": "CRON",
                "target_kind": "PROMPT"
            }))
            .expect("input");
        let schedule = crate::db::schedules::create(&ctx.state.db, input)
            .await
            .expect("schedule");
        crate::db::schedules::set_active(&ctx.state.db, &schedule.id, true)
            .await
            .expect("active");

        let recent_past = (Utc::now() - chrono::Duration::seconds(10)).to_rfc3339();
        crate::db::schedules::set_next_run(&ctx.state.db, &schedule.id, Some(&recent_past))
            .await
            .expect("next");
        let tick = ctx.state.cron.tick_once().await.expect("tick offline");
        assert_eq!(tick.enqueued, 0);
        let after = crate::db::schedules::get(&ctx.state.db, &schedule.id)
            .await
            .expect("after");
        assert_eq!(after.last_run_status.as_deref(), Some("SKIPPED_OFFLINE"));

        let old_past = (Utc::now() - chrono::Duration::seconds(120)).to_rfc3339();
        crate::db::schedules::set_next_run(&ctx.state.db, &schedule.id, Some(&old_past))
            .await
            .expect("next old");
        let tick = ctx.state.cron.tick_once().await.expect("tick misfire");
        assert_eq!(tick.enqueued, 0);
        let after = crate::db::schedules::get(&ctx.state.db, &schedule.id)
            .await
            .expect("after misfire");
        assert_eq!(after.last_run_status.as_deref(), Some("SKIPPED_MISFIRE"));
    }

    #[tokio::test]
    async fn run_now_enqueues_and_queue_controls_work() {
        let ctx = crate::test_support::context().await;
        let input: crate::db::schedules::CreateScheduleInput =
            serde_json::from_value(serde_json::json!({
                "workspace_id": ctx.workspace_id,
                "name": "Rotina de fila",
                "command_or_prompt": "Resuma o dia",
                "cron_expression": "0 8 * * *",
                "timezone": "UTC",
                "trigger_type": "CRON",
                "target_kind": "PROMPT"
            }))
            .expect("input");
        let schedule = crate::db::schedules::create(&ctx.state.db, input)
            .await
            .expect("schedule");

        let result = ctx.state.cron.run_now(&schedule.id).await.expect("run now");
        assert_eq!(result["kind"], "queue");
        let job_id = result["job"]["id"].as_str().expect("job id").to_string();

        let status = ctx
            .state
            .queue
            .status(Some(&ctx.workspace_id))
            .await
            .expect("status");
        assert_eq!(status.waiting, 1);
        assert!(!status.paused);

        let listed = ctx
            .state
            .queue
            .list(Some(&ctx.workspace_id))
            .await
            .expect("list");
        assert!(listed.iter().any(|job| job.id == job_id));

        let prioritized = ctx
            .state
            .queue
            .update_priority(&job_id, 4)
            .await
            .expect("priority");
        assert_eq!(prioritized.priority, 4);

        ctx.state.queue.pause("manutenção").await;
        let paused = ctx.state.queue.status(None).await.expect("paused");
        assert!(paused.paused);
        assert_eq!(paused.pause_reason.as_deref(), Some("manutenção"));
        ctx.state.queue.resume().await.expect("resume");
        assert!(!ctx.state.queue.is_paused());

        let cancelled = ctx.state.queue.cancel(&job_id).await.expect("cancel");
        assert_eq!(cancelled.status, "CANCELLED");
        let retried = ctx.state.queue.retry(&job_id).await.expect("retry");
        assert_eq!(retried.status, "QUEUED");
        assert_eq!(retried.retry_count, 1);
        let _ = ctx.state.queue.cancel(&job_id).await.expect("cancel again");
    }

    #[test]
    fn invalid_cron_and_timezone_are_rejected() {
        let now = chrono::Utc::now();
        assert!(next_occurrence("not-a-cron", "UTC", now).is_err());
        assert!(next_occurrence("0 8 * * *", "Marte/Olympus", now).is_err());
    }
}
