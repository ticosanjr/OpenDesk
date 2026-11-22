use crate::error::AppResult;
use opendesk_storage::events::OpenDeskWsEvent;
use opendesk_storage::repos::{schedules, settings, tasks, workspaces};

use chrono::{DateTime, Utc};

use super::{CronEngine, CronTickResult, next_occurrence};

impl CronEngine {
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
                match self.dispatcher.dispatch_task(&task.id).await {
                    Ok(_) => started += 1,
                    Err(error) => {
                        tracing::debug!(task = %task.id, error = %error, "autostart postponed");
                    }
                }
            }
        }
        started
    }
}
