use crate::{app_state::AppState, error::AppResult, integrations::google_calendar};
use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

pub async fn sync_calendars(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "reports": google_calendar::sync_all(&state).await,
    })))
}

pub async fn sync_source(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<google_calendar::SyncReport>> {
    Ok(Json(google_calendar::sync_source(&state, &id).await?))
}
