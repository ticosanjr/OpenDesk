use crate::{
    app_state::AppState,
    db::{
        queue,
        tasks::{self, CreateTaskInput, UpdateTaskInput},
    },
    error::AppResult,
    events::OpenDeskWsEvent,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use std::sync::Arc;

use super::transition::try_autostart;
use super::{resolve_workspace_id, validate_model_ref};

#[derive(Debug, Deserialize, Default)]
pub struct TaskQuery {
    pub workspace_id: Option<String>,
    /// `work` hides routes/recipes/IO tasks from the Kanban.
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MoveBody {
    #[serde(alias = "column_stage")]
    pub status: String,
    pub position_index: Option<i64>,
}

pub async fn list_tasks(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TaskQuery>,
) -> AppResult<Json<Vec<tasks::Task>>> {
    let workspace_id = resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let list = if query.scope.as_deref() == Some("work") {
        tasks::list_work(&state.db, &workspace_id).await?
    } else {
        tasks::list(&state.db, &workspace_id).await?
    };
    Ok(Json(list))
}

pub async fn create_task(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TaskQuery>,
    Json(input): Json<CreateTaskInput>,
) -> AppResult<Json<tasks::Task>> {
    validate_model_ref(input.preferred_model.as_deref())?;
    if let Some(harness) = input.preferred_harness.as_deref() {
        crate::adapters::validate_harness(harness)?;
    }
    let workspace_id = resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let task = tasks::create(&state.db, &workspace_id, input).await?;
    let task = if task.deduplicated {
        task
    } else {
        try_autostart(&state, &task).await.unwrap_or(task)
    };
    Ok(Json(task))
}

pub async fn get_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<tasks::Task>> {
    Ok(Json(tasks::get(&state.db, &id).await?))
}

pub async fn session_packet(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<crate::session_context::SessionPacket>> {
    Ok(Json(crate::session_context::build(&state.db, &id).await?))
}

pub async fn update_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<UpdateTaskInput>,
) -> AppResult<Json<tasks::Task>> {
    validate_model_ref(input.preferred_model.as_deref())?;
    if let Some(harness) = input.preferred_harness.as_deref() {
        crate::adapters::validate_harness(harness)?;
    }
    let task = tasks::update(&state.db, &id, input).await?;
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: task.status.clone(),
        phase_status: task.phase_status.clone(),
    });
    Ok(Json(task))
}

pub async fn delete_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let task = tasks::get(&state.db, &id).await?;
    // A queue job keeps its own prompt payload, so deleting the task alone
    // would leave an orphan run. Cancel active jobs first.
    let jobs = queue::list(&state.db, Some(&task.workspace_id)).await?;
    for job in jobs
        .into_iter()
        .filter(|job| job.task_id.as_deref() == Some(task.id.as_str()))
        .filter(|job| matches!(job.status.as_str(), "RUNNING" | "QUEUED" | "PAUSED"))
    {
        let _ = state.queue.cancel(&job.id).await;
    }
    tasks::delete(&state.db, &id).await?;
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: "DELETED".into(),
        phase_status: None,
    });
    Ok(StatusCode::NO_CONTENT)
}

pub async fn move_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<MoveBody>,
) -> AppResult<Json<tasks::Task>> {
    let task = tasks::move_to(&state.db, &id, &body.status, body.position_index).await?;
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: task.status.clone(),
        phase_status: task.phase_status.clone(),
    });
    Ok(Json(task))
}
