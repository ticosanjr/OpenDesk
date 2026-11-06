use crate::{
    app_state::AppState,
    db::runs::{self, TaskRun},
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::header,
    response::IntoResponse,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct RunQuery {
    pub workspace_id: Option<String>,
    pub task_id: Option<String>,
}

pub async fn list_runs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RunQuery>,
) -> AppResult<Json<Vec<TaskRun>>> {
    let workspace_id = match query.workspace_id {
        Some(id) => id,
        None => crate::api::tasks::resolve_workspace_id(&state, None).await?,
    };
    Ok(Json(
        runs::list(&state.db, &workspace_id, query.task_id.as_deref()).await?,
    ))
}

pub async fn get_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<TaskRun>> {
    Ok(Json(runs::get(&state.db, &id).await?))
}

pub async fn run_log(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let run = runs::get(&state.db, &id).await?;
    let path = run
        .log_path
        .ok_or_else(|| AppError::NotFound("this run has no log file".into()))?;
    let content = std::fs::read(&path)
        .map_err(|error| AppError::Internal(format!("could not read run log: {error}")))?;
    let start = content.len().saturating_sub(256 * 1024);
    let body = String::from_utf8_lossy(&content[start..]).to_string();
    Ok(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body))
}
