use crate::{
    error::{AppError, AppResult},
    whatsapp::{WHATSAPP_SECRET_NAMESPACE, WhatsAppConfig, connected, load_config},
};
use opendesk_storage::secrets::SecretStore;
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

const DEFAULT_GRAPH_URL: &str = "https://graph.facebook.com";
const DEFAULT_API_VERSION: &str = "v21.0";

#[derive(Debug, Clone, Serialize)]
pub struct SentMessage {
    /// Meta message id; `None` when the response omits it.
    pub message_id: Option<String>,
    pub to: String,
}

fn graph_base() -> String {
    std::env::var("OPENDESK_WHATSAPP_GRAPH_URL").unwrap_or_else(|_| DEFAULT_GRAPH_URL.into())
}

fn api_version(config: &WhatsAppConfig) -> &str {
    if config.api_version.trim().is_empty() {
        DEFAULT_API_VERSION
    } else {
        config.api_version.trim()
    }
}

/// Sends one plain-text message through the Meta Graph API.
///
/// The configuration and connection checks run here so callers only need the
/// database pool and the secret store.
pub async fn send_text(
    pool: &SqlitePool,
    secrets: &SecretStore,
    to: &str,
    text: &str,
) -> AppResult<SentMessage> {
    let config = load_config(pool).await?;
    if !connected(secrets, &config)? {
        return Err(AppError::Conflict(
            "WhatsApp Cloud API não está conectado (provider, phone_number_id, token e enabled)"
                .into(),
        ));
    }
    let to = to.trim();
    let text = text.trim();
    if to.is_empty() || text.is_empty() {
        return Err(AppError::BadRequest(
            "destinatário e texto são obrigatórios".into(),
        ));
    }
    let access_token = secrets
        .get(WHATSAPP_SECRET_NAMESPACE, "access_token")?
        .ok_or_else(|| AppError::Conflict("access token do WhatsApp ausente".into()))?;
    let base = graph_base();
    let url = format!(
        "{}/{}/{}/messages",
        base.trim_end_matches('/'),
        api_version(&config),
        config.phone_number_id.trim()
    );
    let response = reqwest::Client::new()
        .post(&url)
        .bearer_auth(access_token)
        .json(&json!({
            "messaging_product": "whatsapp",
            "to": to,
            "type": "text",
            "text": { "preview_url": false, "body": text }
        }))
        .send()
        .await
        .map_err(|error| AppError::Internal(format!("Graph API inacessível: {error}")))?;
    let status = response.status();
    let value: Value = response.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Err(AppError::Internal(format!(
            "Graph API recusou ({status}): {}",
            value
                .pointer("/error/message")
                .and_then(Value::as_str)
                .unwrap_or("erro desconhecido")
        )));
    }
    Ok(SentMessage {
        message_id: value
            .pointer("/messages/0/id")
            .and_then(Value::as_str)
            .map(str::to_string),
        to: to.to_string(),
    })
}
