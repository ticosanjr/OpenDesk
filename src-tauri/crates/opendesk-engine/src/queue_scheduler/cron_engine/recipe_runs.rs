use crate::{
    config_sync::{commands, paths::StudioPaths},
    dsh_router::{self, PackContext, RouterInput},
    error::{AppError, AppResult},
};
use opendesk_storage::repos::{
    missed_runs,
    queue::{EnqueueInput, QueueJob},
    router as router_db,
    schedules::{self, Schedule},
    settings, workspaces,
};

use chrono::{DateTime, Utc};
use std::path::{Path, PathBuf};

use super::{CronEngine, next_occurrence, render_output_folder};

impl CronEngine {
    pub(super) async fn run_recipe_schedule(
        &self,
        schedule: &Schedule,
        scheduled_for: &str,
    ) -> AppResult<serde_json::Value> {
        let runner = self.recipe_runner.get().ok_or_else(|| {
            AppError::Internal("recipe runner indisponível para a receita".into())
        })?;
        runner.run_schedule(schedule, scheduled_for).await
    }

    /// Enqueue a prompt routine immediately without touching its next run.
    /// Record every missed occurrence between `from` and `now` (bounded) and
    /// apply the user's missed-run policy.
    pub(super) async fn record_missed_occurrences(
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

    pub(super) async fn enqueue_schedule(
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
            crate::browse::ensure_for_task(
                &self.pool,
                &self.paths,
                &schedule.workspace_id,
                &schedule.name,
                &prompt,
                &[],
            )
            .await
            .unwrap_or(true)
        } else {
            false
        };
        let personal_context =
            crate::personal_context::render_body(&self.pool, &schedule.workspace_id)
                .await
                .unwrap_or_default();
        let memory_context = opendesk_storage::repos::memory::render_context(
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
                harness_id: &decision.harness,
                untrusted_data: "",
                output_schema: None,
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
