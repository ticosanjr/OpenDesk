use crate::{
    app_state::AppState,
    db::runs::{self, TaskRun},
    db::schedules,
    error::AppResult,
};
use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

pub async fn schedule_runs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<TaskRun>>> {
    schedules::get(&state.db, &id).await?;
    Ok(Json(runs::list_by_schedule(&state.db, &id).await?))
}
