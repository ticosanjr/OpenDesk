use crate::{app_state::AppState, error::AppResult, queue_scheduler::cron_engine};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

pub async fn run_schedule_now(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    crate::api::engine::require_engine(&state).await?;
    let result = state.cron.run_now(&id).await?;
    Ok(Json(serde_json::json!({ "ok": true, "result": result })))
}

#[derive(Debug, Deserialize)]
pub struct TickQuery {
    #[allow(dead_code)]
    pub force: Option<bool>,
}

pub async fn tick_scheduler(
    State(state): State<Arc<AppState>>,
    Query(_query): Query<TickQuery>,
) -> AppResult<Json<cron_engine::CronTickResult>> {
    let result = state.cron.tick_once().await?;
    Ok(Json(result))
}
