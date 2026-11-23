//! Discord channel API.

use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};
use axum::{Json, extract::State};
use opendesk_channels::discord::{self as channel, DiscordConfig};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct DiscordUpdate {
    #[serde(flatten)]
    pub config: DiscordConfig,
    #[serde(default)]
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DiscordSend {
    pub channel_id: String,
    pub text: String,
}

pub async fn status(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let config = channel::load(&state.db).await?;
    Ok(Json(json!({
        "config": config,
        "token_set": channel::token(&state.secrets).is_some(),
        "running": state.discord.is_running().await,
    })))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DiscordUpdate>,
) -> AppResult<Json<Value>> {
    channel::save(&state.db, &body.config).await?;
    if let Some(token) = body
        .token
        .as_deref()
        .filter(|token| !token.trim().is_empty())
    {
        channel::set_token(&state.secrets, token)?;
    }
    if body.config.enabled {
        state.discord.start_if_enabled(state.clone()).await?;
    } else {
        state.discord.stop().await;
    }
    status(State(state)).await
}

pub async fn start(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    state.discord.start_if_enabled(state.clone()).await?;
    status(State(state)).await
}

pub async fn stop(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    state.discord.stop().await;
    status(State(state)).await
}

pub async fn send(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DiscordSend>,
) -> AppResult<Json<Value>> {
    let token = channel::token(&state.secrets)
        .ok_or_else(|| AppError::BadRequest("token do Discord ausente".into()))?;
    channel::send_text(&token, &body.channel_id, &body.text).await?;
    Ok(Json(json!({ "ok": true })))
}
