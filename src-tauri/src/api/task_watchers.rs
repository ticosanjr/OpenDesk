use crate::{
    app_state::AppState,
    db::{
        task_watchers::{self, TaskWatcher},
        tasks,
    },
    error::AppResult,
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct WatchersBody {
    #[serde(default)]
    pub channels: Vec<String>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<String>,
) -> AppResult<Json<Vec<TaskWatcher>>> {
    tasks::get(&state.db, &task_id).await?;
    Ok(Json(
        task_watchers::list_for_task(&state.db, &task_id).await?,
    ))
}

pub async fn replace(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<String>,
    Json(body): Json<WatchersBody>,
) -> AppResult<Json<Vec<TaskWatcher>>> {
    let task = tasks::get(&state.db, &task_id).await?;
    Ok(Json(
        task_watchers::replace(&state.db, &task.workspace_id, &task_id, &body.channels).await?,
    ))
}
