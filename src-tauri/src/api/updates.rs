use crate::{
    app_state::AppState,
    error::AppResult,
    updates::{self, DownloadResult, UpdateInfo},
};
use axum::{Json, extract::State};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct TokenBody {
    pub token: String,
}

pub async fn status(State(state): State<Arc<AppState>>) -> Json<UpdateInfo> {
    Json(updates::check(&state).await)
}

pub async fn check(State(state): State<Arc<AppState>>) -> Json<UpdateInfo> {
    Json(updates::check(&state).await)
}

pub async fn put_token(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TokenBody>,
) -> AppResult<Json<UpdateInfo>> {
    updates::save_token(&state, &body.token)?;
    Ok(Json(updates::check(&state).await))
}

pub async fn delete_token(State(state): State<Arc<AppState>>) -> AppResult<Json<UpdateInfo>> {
    updates::save_token(&state, "")?;
    Ok(Json(updates::check(&state).await))
}

pub async fn download(State(state): State<Arc<AppState>>) -> AppResult<Json<DownloadResult>> {
    Ok(Json(updates::download_and_stage(&state).await?))
}
