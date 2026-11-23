use crate::{
    app_state::AppState,
    db::{queue, settings, tasks},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use std::sync::Arc;

use super::{DispatchBody, dispatch_task_internal};

#[derive(Debug, Deserialize)]
pub struct TransitionBody {
    #[serde(alias = "column_stage")]
    pub status: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HoldBody {
    pub held: bool,
}

async fn cancel_active_jobs(state: &AppState, task: &tasks::Task) {
    let jobs = queue::list(&state.db, Some(&task.workspace_id))
        .await
        .unwrap_or_default();
    for job in jobs
        .into_iter()
        .filter(|job| job.task_id.as_deref() == Some(task.id.as_str()))
        .filter(|job| matches!(job.status.as_str(), "RUNNING" | "QUEUED" | "PAUSED"))
    {
        let _ = state.queue.cancel(&job.id).await;
    }
}

async fn autostart_enabled(state: &AppState) -> bool {
    settings::bool_flag(&state.db, "tasks_autostart", true).await
}

/// Enqueue a READY task when auto-start is enabled and the engine is usable.
/// Never fails the surrounding operation: if the engine is offline, the task
/// stays in the queue and the reconciler retries later.
pub(super) async fn try_autostart(
    state: &Arc<AppState>,
    task: &tasks::Task,
) -> Option<tasks::Task> {
    if task.status != "TODO" || task.held {
        return None;
    }
    if matches!(
        task.phase_status.as_deref(),
        Some("awaiting_approval") | Some("queued") | Some("follow_up_queued") | Some("running")
    ) {
        return None;
    }
    if !autostart_enabled(state).await || state.queue.is_paused() {
        return None;
    }
    if let Err(error) = dispatch_task_internal(state, &task.id, DispatchBody::default(), None).await
    {
        tracing::debug!(task = %task.id, error = %error, "task autostart postponed");
        return None;
    }
    tasks::get(&state.db, &task.id).await.ok()
}

/// Explicit status transition. Moving a card to Em execução dispatches a real
/// job; moving to Fila re-opens and auto-starts it; terminal states cancel any
/// queued/running job before changing the column.
pub async fn transition_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<TransitionBody>,
) -> AppResult<Json<serde_json::Value>> {
    let task = tasks::get(&state.db, &id).await?;
    let target = body.status.trim().to_uppercase();

    match target.as_str() {
        "BACKLOG" | "TODO" => {
            cancel_active_jobs(&state, &task).await;
            tasks::set_held(&state.db, &id, false).await?;
            tasks::set_blocked_reason(&state.db, &id, None).await?;
            let task = tasks::replace_phase_status(&state.db, &id, None, Some("TODO")).await?;
            publish_task(&state, &task);
            let task = try_autostart(&state, &task).await.unwrap_or(task);
            publish_task(&state, &task);
            Ok(Json(serde_json::json!({ "task": task, "job": null })))
        }
        "CANCELLED" => {
            cancel_active_jobs(&state, &task).await;
            let task = tasks::set_cancelled(&state.db, &id).await?;
            tasks::set_held(&state.db, &id, false).await?;
            publish_task(&state, &task);
            Ok(Json(serde_json::json!({ "task": task, "job": null })))
        }
        "BLOCKED" => {
            cancel_active_jobs(&state, &task).await;
            tasks::set_held(&state.db, &id, false).await?;
            tasks::set_blocked_reason(
                &state.db,
                &id,
                Some(body.reason.as_deref().unwrap_or("Bloqueada manualmente")),
            )
            .await?;
            let task =
                tasks::replace_phase_status(&state.db, &id, Some("blocked"), Some("BLOCKED"))
                    .await?;
            publish_task(&state, &task);
            Ok(Json(serde_json::json!({ "task": task, "job": null })))
        }
        "DONE" => {
            cancel_active_jobs(&state, &task).await;
            tasks::set_held(&state.db, &id, false).await?;
            tasks::set_blocked_reason(&state.db, &id, None).await?;
            let task =
                tasks::replace_phase_status(&state.db, &id, Some("done"), Some("DONE")).await?;
            publish_task(&state, &task);
            Ok(Json(serde_json::json!({ "task": task, "job": null })))
        }
        "IN_REVIEW" => {
            cancel_active_jobs(&state, &task).await;
            tasks::set_held(&state.db, &id, false).await?;
            let task = tasks::replace_phase_status(
                &state.db,
                &id,
                Some("awaiting_input"),
                Some("IN_REVIEW"),
            )
            .await?;
            publish_task(&state, &task);
            Ok(Json(serde_json::json!({ "task": task, "job": null })))
        }
        "IN_PROGRESS" => {
            if task.active_job_status.is_some() {
                return Ok(Json(serde_json::json!({ "task": task, "job": null })));
            }
            crate::api::engine::require_engine_strict(&state).await?;
            dispatch_task_internal(&state, &id, DispatchBody::default(), None).await?;
            let task = tasks::get(&state.db, &id).await?;
            publish_task(&state, &task);
            Ok(Json(serde_json::json!({ "task": task, "job": null })))
        }
        other => Err(AppError::BadRequest(format!(
            "invalid task status '{other}'"
        ))),
    }
}

pub async fn hold_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<HoldBody>,
) -> AppResult<Json<tasks::Task>> {
    let task = tasks::get(&state.db, &id).await?;
    if body.held {
        cancel_active_jobs(&state, &task).await;
        tasks::set_blocked_reason(&state.db, &id, None).await?;
        tasks::replace_phase_status(&state.db, &id, None, Some("TODO")).await?;
        tasks::set_held(&state.db, &id, true).await?;
        let task = tasks::get(&state.db, &id).await?;
        publish_task(&state, &task);
        Ok(Json(task))
    } else {
        tasks::set_held(&state.db, &id, false).await?;
        let task = tasks::replace_phase_status(&state.db, &id, None, Some("TODO")).await?;
        publish_task(&state, &task);
        let task = try_autostart(&state, &task).await.unwrap_or(task);
        publish_task(&state, &task);
        Ok(Json(task))
    }
}

fn publish_task(state: &AppState, task: &tasks::Task) {
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: task.status.clone(),
        phase_status: task.phase_status.clone(),
    });
}
