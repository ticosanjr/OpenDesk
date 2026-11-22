use super::{QueueManager, forces_worktree, split_model, worktree_runs_enabled};
use crate::opencode_client::session_runner::{self, RunRequest, RunStatus};
use crate::queue_scheduler::{run_execution, settlement};
use opendesk_storage::events::OpenDeskWsEvent;
use opendesk_storage::repos::{budget, model_health, queue, runs, tasks, workspaces};
use opendesk_storage::secrets::SecretStore;

use std::path::PathBuf;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

struct HeartbeatGuard {
    stop: CancellationToken,
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for HeartbeatGuard {
    fn drop(&mut self) {
        self.stop.cancel();
        self.handle.abort();
    }
}

impl QueueManager {
    pub(super) async fn execute_job(&self, job: queue::QueueJob) {
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
            let write_buffer = self.write_buffer.clone();
            let job_id = job.id.clone();
            let stop = heartbeat_stop.clone();
            tokio::spawn(async move {
                loop {
                    tokio::select! {
                        _ = stop.cancelled() => break,
                        _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                            let _ = write_buffer.job_heartbeat(&job_id).await;
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
        let worktree_enabled = worktree_runs_enabled(&self.pool).await
            || forces_worktree(task.as_ref().map(|task| task.tags_json.as_str()));
        let (plan, worktree) = if worktree_enabled {
            let prepared = match crate::worktree::prepare(
                &job.workspace_id,
                &workspace_path,
                &self.paths.data_dir,
                None,
            ) {
                Ok(worktree) => worktree,
                Err(error) => {
                    let message = format!("worktree indisponível: {error:#}");
                    let _ = queue::finish(&self.pool, &job.id, "FAILED", Some(&message)).await;
                    settlement::mark_schedule(&self.pool, &self.events, &job, "FAILED").await;
                    self.publish_queue(&job.workspace_id).await;
                    return;
                }
            };
            let execution_root = PathBuf::from(&prepared.path);
            let plan = run_execution::plan_run(&job, task.as_ref(), &self.paths, &execution_root);
            let mut stored = prepared;
            stored.run_id = Some(plan.run_id.clone());
            if let Err(error) = crate::worktree::store::insert(&self.pool, &stored).await {
                tracing::warn!(error = %error, "could not persist run worktree");
            }
            (plan, Some(stored))
        } else {
            (
                run_execution::plan_run(&job, task.as_ref(), &self.paths, &workspace_path),
                None,
            )
        };
        let execution_root = worktree
            .as_ref()
            .map(|worktree| PathBuf::from(&worktree.path))
            .unwrap_or_else(|| workspace_path.clone());
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
            .unwrap_or_else(|poison| poison.into_inner())
            .insert(job.id.clone(), cancel.clone());

        let started = Instant::now();
        let secrets = SecretStore::new(&self.paths);
        let request = RunRequest {
            notifier: Some(self.notifier.as_ref()),
            pool: &self.pool,
            secrets: &secrets,
            workspace_id: &job.workspace_id,
            run_id: &run_id,
            task_id: job.task_id.as_deref(),
            session_id: job.session_id.as_deref(),
            workspace_root: &execution_root,
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
            .unwrap_or_else(|poison| poison.into_inner())
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
            &execution_root,
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

        if let Some(task_id) = job.task_id.as_deref()
            && let Ok(Some(handoff)) = opendesk_storage::repos::handoffs::complete_for_task(
                &self.pool,
                task_id,
                outcome.status == RunStatus::Success,
            )
            .await
        {
            self.events.publish(OpenDeskWsEvent::HandoffUpdated {
                handoff_id: handoff.id,
                workspace_id: handoff.workspace_id,
                status: handoff.status,
                target_agent: handoff.target_agent,
            });
        }

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

        let run_success = outcome.status == RunStatus::Success;
        if let Err(error) = self
            .ports
            .after_run(
                &self.pool,
                &self.paths,
                &self.events,
                &job,
                &run_id,
                run_success,
            )
            .await
        {
            tracing::warn!(error = %error, "post-run queue ports failed");
        }

        self.running_jobs
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
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
}
