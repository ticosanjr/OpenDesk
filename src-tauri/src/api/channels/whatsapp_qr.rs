//! WhatsApp Web QR sidecar API.

use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    integrations::whatsapp_qr as host,
};
use axum::{Json, extract::State};
use opendesk_channels::whatsapp::{WhatsAppQrConfig, save_qr_config};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct QrUpdate {
    #[serde(flatten)]
    pub config: WhatsAppQrConfig,
}

#[derive(Debug, Deserialize)]
pub struct QrSend {
    pub to: String,
    pub text: String,
}

pub async fn status(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    Ok(Json(host::status(&state).await?))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Json(body): Json<QrUpdate>,
) -> AppResult<Json<Value>> {
    save_qr_config(&state.db, &body.config).await?;
    if body.config.enabled {
        state.whatsapp_qr.start_if_enabled(state.clone()).await?;
    } else {
        state.whatsapp_qr.stop().await;
    }
    status(State(state)).await
}

pub async fn start(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    state.whatsapp_qr.start_if_enabled(state.clone()).await?;
    status(State(state)).await
}

pub async fn stop(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    state.whatsapp_qr.stop().await;
    status(State(state)).await
}

pub async fn send(
    State(state): State<Arc<AppState>>,
    Json(body): Json<QrSend>,
) -> AppResult<Json<Value>> {
    if body.to.trim().is_empty() {
        return Err(AppError::BadRequest("destinatário vazio".into()));
    }
    state.whatsapp_qr.send_text(&body.to, &body.text).await?;
    Ok(Json(json!({ "ok": true })))
}
