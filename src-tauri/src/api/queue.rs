use crate::{app_state::AppState, db::queue::QueueJob, error::AppResult, events::QueueJobDto};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct QueueQuery {
    pub workspace_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct PauseQuery {
    pub reason: Option<String>,
}

pub async fn list_queue(
    State(state): State<Arc<AppState>>,
    Query(query): Query<QueueQuery>,
) -> AppResult<Json<Vec<QueueJobDto>>> {
    Ok(Json(state.queue.list(query.workspace_id.as_deref()).await?))
}

pub async fn queue_status(
    State(state): State<Arc<AppState>>,
    Query(query): Query<QueueQuery>,
) -> AppResult<Json<crate::queue_scheduler::QueueStatus>> {
    Ok(Json(
        state.queue.status(query.workspace_id.as_deref()).await?,
    ))
}

pub async fn pause_queue(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PauseQuery>,
) -> AppResult<Json<serde_json::Value>> {
    state
        .queue
        .pause(query.reason.unwrap_or_else(|| "paused by user".into()))
        .await;
    Ok(Json(serde_json::json!({ "ok": true, "paused": true })))
}

pub async fn resume_queue(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<serde_json::Value>> {
    state.queue.resume().await?;
    Ok(Json(serde_json::json!({ "ok": true, "paused": false })))
}

pub async fn cancel_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<QueueJob>> {
    Ok(Json(state.queue.cancel(&id).await?))
}

pub async fn retry_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<QueueJob>> {
    Ok(Json(state.queue.retry(&id).await?))
}

#[derive(Debug, Deserialize)]
pub struct PriorityBody {
    pub priority: i64,
}

pub async fn update_priority(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<PriorityBody>,
) -> AppResult<Json<QueueJob>> {
    Ok(Json(state.queue.update_priority(&id, body.priority).await?))
}
