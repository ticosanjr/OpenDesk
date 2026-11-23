//! Slack channel API.

use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};
use axum::{Json, extract::State};
use opendesk_channels::slack::{self as channel, SlackConfig};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct SlackUpdate {
    #[serde(flatten)]
    pub config: SlackConfig,
    #[serde(default)]
    pub app_token: Option<String>,
    #[serde(default)]
    pub bot_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SlackSend {
    pub channel_id: String,
    pub text: String,
}

pub async fn status(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let config = channel::load(&state.db).await?;
    Ok(Json(json!({
        "config": config,
        "app_token_set": channel::app_token(&state.secrets).is_some(),
        "bot_token_set": channel::bot_token(&state.secrets).is_some(),
        "running": state.slack.is_running().await,
    })))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SlackUpdate>,
) -> AppResult<Json<Value>> {
    channel::save(&state.db, &body.config).await?;
    if let (Some(app), Some(bot)) = (body.app_token.as_deref(), body.bot_token.as_deref()) {
        channel::set_tokens(&state.secrets, app, bot)?;
    } else if let Some(app) = body.app_token.as_deref() {
        channel::set_tokens(&state.secrets, app, "")?;
    } else if let Some(bot) = body.bot_token.as_deref() {
        channel::set_tokens(&state.secrets, "", bot)?;
    }
    if body.config.enabled {
        state.slack.start_if_enabled(state.clone()).await?;
    } else {
        state.slack.stop().await;
    }
    status(State(state)).await
}

pub async fn start(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    state.slack.start_if_enabled(state.clone()).await?;
    status(State(state)).await
}

pub async fn stop(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    state.slack.stop().await;
    status(State(state)).await
}

pub async fn send(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SlackSend>,
) -> AppResult<Json<Value>> {
    let token = channel::bot_token(&state.secrets)
        .ok_or_else(|| AppError::BadRequest("bot token do Slack ausente".into()))?;
    channel::send_text(&token, &body.channel_id, &body.text).await?;
    Ok(Json(json!({ "ok": true })))
}
