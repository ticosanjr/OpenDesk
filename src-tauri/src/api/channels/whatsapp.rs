use crate::{app_state::AppState, error::AppResult};
use axum::{Json, extract::State};
use opendesk_channels::whatsapp::{
    self, WEBHOOK_NAMESPACE, WHATSAPP_SECRET_NAMESPACE, WhatsAppConfig,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn get_whatsapp(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let config = whatsapp::load_config(&state.db).await?;
    let token_set = state.secrets.get(WEBHOOK_NAMESPACE, "whatsapp")?.is_some();
    let access_token_set = state
        .secrets
        .get(WHATSAPP_SECRET_NAMESPACE, "access_token")?
        .is_some();
    let app_secret_set = state
        .secrets
        .get(WHATSAPP_SECRET_NAMESPACE, "app_secret")?
        .is_some();
    let connected = whatsapp::connected(&state.secrets, &config)?;
    Ok(Json(json!({
        "config": config,
        "connected": connected,
        "connector": if config.provider == "cloud-api" { "cloud-api" } else { "structure-only" },
        "webhook_token_set": token_set,
        "access_token_set": access_token_set,
        "app_secret_set": app_secret_set,
    })))
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppUpdate {
    #[serde(flatten)]
    pub config: WhatsAppConfig,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub app_secret: Option<String>,
}

pub async fn put_whatsapp(
    State(state): State<Arc<AppState>>,
    Json(body): Json<WhatsAppUpdate>,
) -> AppResult<Json<Value>> {
    whatsapp::validate_config(&body.config)?;
    whatsapp::apply_credentials(
        &state.secrets,
        body.access_token.as_deref(),
        body.app_secret.as_deref(),
    )?;
    whatsapp::save_config(&state.db, &body.config).await?;
    let connected = whatsapp::connected(&state.secrets, &body.config)?;
    Ok(Json(json!({
        "config": body.config,
        "connected": connected,
        "connector": "cloud-api",
        "access_token_set": state.secrets.get(WHATSAPP_SECRET_NAMESPACE, "access_token")?.is_some(),
        "app_secret_set": state.secrets.get(WHATSAPP_SECRET_NAMESPACE, "app_secret")?.is_some(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppSend {
    pub to: String,
    pub text: String,
}

pub async fn send_whatsapp(
    State(state): State<Arc<AppState>>,
    Json(body): Json<WhatsAppSend>,
) -> AppResult<Json<Value>> {
    let sent = whatsapp::send_text(&state.db, &state.secrets, &body.to, &body.text).await?;
    Ok(Json(json!({
        "message_id": sent.message_id,
        "to": sent.to,
    })))
}
