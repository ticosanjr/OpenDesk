use crate::{app_state::AppState, dedupe, error::AppResult};
use axum::{Json, extract::State};
use std::sync::Arc;

pub async fn get(State(state): State<Arc<AppState>>) -> AppResult<Json<dedupe::DedupeSettings>> {
    Ok(Json(dedupe::get_settings(&state.db).await?))
}

pub async fn put(
    State(state): State<Arc<AppState>>,
    Json(settings): Json<dedupe::DedupeSettings>,
) -> AppResult<Json<dedupe::DedupeSettings>> {
    Ok(Json(dedupe::set_settings(&state.db, settings).await?))
}
