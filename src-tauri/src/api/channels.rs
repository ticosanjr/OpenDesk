use crate::{
    app_state::AppState,
    db::workspaces,
    db::{
        channels::{self, ChannelRoute, UpsertRouteInput},
        tasks,
        tickets::{self, CreateTicketInput},
    },
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use hmac::{Hmac, KeyInit, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::Arc;

const WHATSAPP_SETTING: &str = "channels.whatsapp";
const WHATSAPP_SECRET_NAMESPACE: &str = "channel_whatsapp";
const WEBHOOK_NAMESPACE: &str = "channel_webhook";
const WEBHOOK_HEADER: &str = "x-opendesk-channel-token";
const MAX_TEXT: usize = 16_000;

#[derive(Debug, Deserialize, Default)]
pub struct WorkspaceQuery {
    pub workspace_id: Option<String>,
}

pub async fn list_routes(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> AppResult<Json<Vec<ChannelRoute>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(channels::list(&state.db, &workspace_id).await?))
}

pub async fn upsert_route(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
    Json(input): Json<UpsertRouteInput>,
) -> AppResult<Json<ChannelRoute>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        channels::upsert(&state.db, &workspace_id, input).await?,
    ))
}

pub async fn delete_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    if !channels::delete(&state.db, &id).await? {
        return Err(AppError::NotFound(format!("channel route {id}")));
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WhatsAppConfig {
    pub enabled: bool,
    /// `structure`, `cloud-api` or `baileys`.
    pub provider: String,
    pub phone_number: String,
    /// Meta Cloud API phone number id.
    pub phone_number_id: String,
    pub business_account_id: String,
    pub api_version: String,
    /// Optional workspace for inbound messages; defaults to the active one.
    pub workspace_id: Option<String>,
    pub notes: String,
}

impl Default for WhatsAppConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "structure".into(),
            phone_number: String::new(),
            phone_number_id: String::new(),
            business_account_id: String::new(),
            api_version: "v21.0".into(),
            workspace_id: None,
            notes: String::new(),
        }
    }
}

async fn whatsapp_config(state: &AppState) -> AppResult<WhatsAppConfig> {
    Ok(crate::db::settings::get(&state.db, WHATSAPP_SETTING)
        .await?
        .and_then(|value| serde_json::from_value::<WhatsAppConfig>(value).ok())
        .unwrap_or_default())
}

fn whatsapp_connected(state: &AppState, config: &WhatsAppConfig) -> AppResult<bool> {
    Ok(config.enabled
        && config.provider == "cloud-api"
        && !config.phone_number_id.trim().is_empty()
        && state
            .secrets
            .get(WHATSAPP_SECRET_NAMESPACE, "access_token")?
            .is_some())
}

pub async fn get_whatsapp(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let config = whatsapp_config(&state).await?;
    let token_set = state.secrets.get(WEBHOOK_NAMESPACE, "whatsapp")?.is_some();
    let access_token_set = state
        .secrets
        .get(WHATSAPP_SECRET_NAMESPACE, "access_token")?
        .is_some();
    let app_secret_set = state
        .secrets
        .get(WHATSAPP_SECRET_NAMESPACE, "app_secret")?
        .is_some();
    let connected = whatsapp_connected(&state, &config)?;
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
    if !["structure", "cloud-api", "baileys"].contains(&body.config.provider.as_str()) {
        return Err(AppError::BadRequest(
            "provider de WhatsApp inválido; use structure, cloud-api ou baileys".into(),
        ));
    }
    if body.config.provider == "cloud-api" && body.config.phone_number_id.trim().is_empty() {
        return Err(AppError::BadRequest(
            "cloud-api exige phone_number_id (Meta)".into(),
        ));
    }
    for (key, value) in [
        ("access_token", body.access_token.as_deref()),
        ("app_secret", body.app_secret.as_deref()),
    ] {
        match value.map(str::trim) {
            Some("") => state.secrets.delete(WHATSAPP_SECRET_NAMESPACE, key)?,
            Some(value) => state.secrets.set(WHATSAPP_SECRET_NAMESPACE, key, value)?,
            None => {}
        }
    }
    crate::db::settings::set(
        &state.db,
        WHATSAPP_SETTING,
        &serde_json::to_value(&body.config)
            .map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .await?;
    let connected = whatsapp_connected(&state, &body.config)?;
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
    let config = whatsapp_config(&state).await?;
    if !whatsapp_connected(&state, &config)? {
        return Err(AppError::Conflict(
            "WhatsApp Cloud API não está conectado (provider, phone_number_id, token e enabled)"
                .into(),
        ));
    }
    let to = body.to.trim();
    let text = body.text.trim();
    if to.is_empty() || text.is_empty() {
        return Err(AppError::BadRequest(
            "destinatário e texto são obrigatórios".into(),
        ));
    }
    let access_token = state
        .secrets
        .get(WHATSAPP_SECRET_NAMESPACE, "access_token")?
        .ok_or_else(|| AppError::Conflict("access token do WhatsApp ausente".into()))?;
    let base = std::env::var("OPENDESK_WHATSAPP_GRAPH_URL")
        .unwrap_or_else(|_| "https://graph.facebook.com".into());
    let version = if config.api_version.trim().is_empty() {
        "v21.0"
    } else {
        config.api_version.trim()
    };
    let url = format!(
        "{}/{}/{}/messages",
        base.trim_end_matches('/'),
        version,
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
    Ok(Json(json!({
        "message_id": value
            .pointer("/messages/0/id")
            .and_then(Value::as_str),
        "to": to,
    })))
}

#[derive(Debug, Serialize)]
pub struct ChannelStatus {
    pub channel: String,
    pub routes: usize,
    pub webhook_token_set: bool,
    pub webhook_path: String,
    pub connector: String,
}

pub async fn status(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> AppResult<Json<Vec<ChannelStatus>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let routes = channels::list(&state.db, &workspace_id).await?;
    let whatsapp = whatsapp_config(&state).await?;
    let whatsapp_connected = whatsapp_connected(&state, &whatsapp)?;
    let list = channels::CHANNELS
        .iter()
        .map(|channel| ChannelStatus {
            channel: (*channel).to_string(),
            routes: routes
                .iter()
                .filter(|route| route.channel == *channel)
                .count(),
            webhook_token_set: state
                .secrets
                .get(WEBHOOK_NAMESPACE, channel)
                .ok()
                .flatten()
                .is_some(),
            webhook_path: format!("/api/webhooks/channels/{channel}"),
            connector: if *channel == "whatsapp" {
                if whatsapp_connected {
                    "cloud-api".into()
                } else if whatsapp.provider == "cloud-api" {
                    "cloud-api (incompleto)".into()
                } else {
                    "structure-only".into()
                }
            } else if matches!(*channel, "telegram" | "signal") {
                "native-bridge".into()
            } else {
                "webhook".into()
            },
        })
        .collect();
    Ok(Json(list))
}

/// Generates (and returns once) the shared secret used by a channel webhook.
pub async fn rotate_webhook_token(
    State(state): State<Arc<AppState>>,
    Path(channel): Path<String>,
) -> AppResult<Json<Value>> {
    let channel = channels::validate_channel(&channel)?;
    let mut bytes = [0u8; 24];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let token = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    state.secrets.set(WEBHOOK_NAMESPACE, &channel, &token)?;
    Ok(Json(json!({
        "channel": channel,
        "token": token,
        "header": WEBHOOK_HEADER,
        "webhook_path": format!("/api/webhooks/channels/{channel}"),
    })))
}

/// Meta webhook verification: `GET /api/webhooks/channels/whatsapp?hub.*`.
pub async fn verify_webhook(
    State(state): State<Arc<AppState>>,
    Path(channel): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> axum::response::Response {
    let result = async {
        let channel = channels::validate_channel(&channel)?;
        let mode = query.get("hub.mode").map(String::as_str).unwrap_or("");
        let token = query
            .get("hub.verify_token")
            .map(String::as_str)
            .unwrap_or("");
        let challenge = query.get("hub.challenge").cloned().unwrap_or_default();
        if mode != "subscribe" {
            return Err(AppError::BadRequest("hub.mode inválido".into()));
        }
        let expected = state
            .secrets
            .get(WEBHOOK_NAMESPACE, &channel)?
            .ok_or_else(|| AppError::BadRequest("canal sem token de webhook".into()))?;
        if token != expected {
            return Err(AppError::BadRequest("verify_token inválido".into()));
        }
        Ok(challenge)
    }
    .await;
    match result {
        Ok(challenge) => (StatusCode::OK, challenge).into_response(),
        Err(error) => error.into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct InboundBody {
    #[serde(default)]
    pub identity: String,
    pub text: String,
    #[serde(default)]
    pub message_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    /// `true` marks the message as already read by the bridge. Informational.
    #[serde(default)]
    pub already_seen: bool,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone)]
struct MetaMessage {
    identity: String,
    text: String,
    message_id: Option<String>,
    name: Option<String>,
    media_id: Option<String>,
    media_mime: Option<String>,
    media_name: Option<String>,
}

fn extract_meta_messages(value: &Value) -> Vec<MetaMessage> {
    let mut messages = Vec::new();
    let Some(entries) = value.get("entry").and_then(Value::as_array) else {
        return messages;
    };
    for entry in entries {
        let Some(changes) = entry.get("changes").and_then(Value::as_array) else {
            continue;
        };
        for change in changes {
            let Some(value) = change.get("value") else {
                continue;
            };
            let contacts = value
                .get("contacts")
                .and_then(Value::as_array)
                .map(|contacts| {
                    contacts
                        .iter()
                        .filter_map(|contact| {
                            let id = contact.get("wa_id").and_then(Value::as_str)?;
                            let name = contact
                                .pointer("/profile/name")
                                .and_then(Value::as_str)
                                .map(str::to_string);
                            Some((id.to_string(), name))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let Some(items) = value.get("messages").and_then(Value::as_array) else {
                continue;
            };
            for message in items {
                let Some(identity) = message.get("from").and_then(Value::as_str) else {
                    continue;
                };
                let kind = message.get("type").and_then(Value::as_str).unwrap_or("");
                let media = message.get(kind).and_then(Value::as_object);
                let text = match kind {
                    "text" => message
                        .pointer("/text/body")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    "button" => message
                        .pointer("/button/text")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    "interactive" => message
                        .pointer("/interactive/button_reply/title")
                        .or_else(|| message.pointer("/interactive/list_reply/title"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    "image" | "document" | "video" | "audio" | "voice" | "sticker" => message
                        .pointer("/image/caption")
                        .or_else(|| message.pointer("/document/caption"))
                        .or_else(|| message.pointer("/video/caption"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("[{kind} recebido]")),
                    "" => continue,
                    other => format!("[{other} recebido]"),
                };
                if text.trim().is_empty() {
                    continue;
                }
                let name = contacts
                    .iter()
                    .find(|(id, _)| id == identity)
                    .and_then(|(_, name)| name.clone());
                messages.push(MetaMessage {
                    identity: identity.to_string(),
                    text,
                    message_id: message
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    name,
                    media_id: media
                        .and_then(|media| media.get("id"))
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    media_mime: media
                        .and_then(|media| media.get("mime_type"))
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    media_name: media
                        .and_then(|media| media.get("filename"))
                        .and_then(Value::as_str)
                        .map(str::to_string),
                });
            }
        }
    }
    messages
}

fn verify_meta_signature(secret: &str, body: &[u8], header: &str) -> bool {
    let Some(encoded) = header.strip_prefix("sha256=") else {
        return false;
    };
    let Some(expected) = hex_decode(encoded) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    mac.finalize().into_bytes().as_slice() == expected.as_slice()
}

fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}

fn media_extension(mime: Option<&str>, filename: Option<&str>) -> String {
    if let Some(filename) = filename
        && let Some(extension) = std::path::Path::new(filename)
            .extension()
            .and_then(|value| value.to_str())
    {
        let extension = extension
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .take(8)
            .collect::<String>();
        if !extension.is_empty() {
            return extension.to_lowercase();
        }
    }
    match mime.unwrap_or("").split(';').next().unwrap_or("") {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "audio/ogg" => "ogg",
        "audio/mpeg" => "mp3",
        "audio/mp4" => "m4a",
        "audio/wav" => "wav",
        "video/mp4" => "mp4",
        "application/pdf" => "pdf",
        _ => "bin",
    }
    .to_string()
}

async fn download_whatsapp_media(
    state: &Arc<AppState>,
    config: &WhatsAppConfig,
    workspace_root: &str,
    message: &MetaMessage,
) -> AppResult<String> {
    let (Some(media_id), Some(access_token)) = (
        message.media_id.as_deref(),
        state
            .secrets
            .get(WHATSAPP_SECRET_NAMESPACE, "access_token")?,
    ) else {
        return Ok(message.text.clone());
    };
    let base = std::env::var("OPENDESK_WHATSAPP_GRAPH_URL")
        .unwrap_or_else(|_| "https://graph.facebook.com".into());
    let version = if config.api_version.trim().is_empty() {
        "v21.0"
    } else {
        config.api_version.trim()
    };
    let client = reqwest::Client::new();
    let metadata = client
        .get(format!(
            "{}/{}/{media_id}",
            base.trim_end_matches('/'),
            version
        ))
        .bearer_auth(&access_token)
        .send()
        .await
        .map_err(|error| AppError::Internal(format!("Graph media inacessível: {error}")))?;
    let status = metadata.status();
    let value: Value = metadata.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        return Ok(message.text.clone());
    }
    if value
        .get("file_size")
        .and_then(Value::as_i64)
        .is_some_and(|size| size > 25 * 1024 * 1024)
    {
        return Ok(format!(
            "{}\n[mídia acima de 25 MB não baixada]",
            message.text
        ));
    }
    let Some(url) = value.get("url").and_then(Value::as_str) else {
        return Ok(message.text.clone());
    };
    let mime = value
        .get("mime_type")
        .and_then(Value::as_str)
        .or(message.media_mime.as_deref());
    let response = client
        .get(url)
        .bearer_auth(&access_token)
        .send()
        .await
        .map_err(|error| AppError::Internal(format!("download da mídia falhou: {error}")))?;
    if !response.status().is_success() {
        return Ok(message.text.clone());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| AppError::Internal(format!("corpo da mídia falhou: {error}")))?;
    if bytes.len() > 25 * 1024 * 1024 {
        return Ok(format!(
            "{}\n[mídia acima de 25 MB não baixada]",
            message.text
        ));
    }
    let extension = media_extension(mime, message.media_name.as_deref());
    let dir = std::path::Path::new(workspace_root).join("input/whatsapp");
    tokio::fs::create_dir_all(&dir).await?;
    let id = message
        .message_id
        .as_deref()
        .unwrap_or("media")
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .take(40)
        .collect::<String>();
    let file = format!(
        "{}-{}.{}",
        chrono::Utc::now().format("%Y%m%d%H%M%S"),
        if id.is_empty() { "media" } else { &id },
        extension
    );
    tokio::fs::write(dir.join(&file), &bytes).await?;
    Ok(format!(
        "{}\n[arquivo salvo em input/whatsapp/{file}]",
        message.text
    ))
}

/// Universal inbound webhook. Generic channels use `x-opendesk-channel-token`;
/// Meta sends a signed `whatsapp_business_account` payload.
pub async fn inbound(
    State(state): State<Arc<AppState>>,
    Path(channel): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> AppResult<Json<Value>> {
    let channel = channels::validate_channel(&channel)?;
    let value: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let is_meta = value.get("object").and_then(Value::as_str) == Some("whatsapp_business_account");

    if is_meta {
        if channel != "whatsapp" {
            return Err(AppError::BadRequest(
                "payload Meta só é aceito no canal whatsapp".into(),
            ));
        }
        let secret = state
            .secrets
            .get(WHATSAPP_SECRET_NAMESPACE, "app_secret")?
            .ok_or_else(|| {
                AppError::BadRequest(
                    "configure o app_secret do WhatsApp para validar o webhook".into(),
                )
            })?;
        let signature = headers
            .get("x-hub-signature-256")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        if !verify_meta_signature(&secret, &body, signature) {
            return Err(AppError::BadRequest(
                "assinatura do webhook inválida".into(),
            ));
        }
        let config = whatsapp_config(&state).await?;
        let workspace_id =
            crate::api::tasks::resolve_workspace_id(&state, config.workspace_id.as_deref()).await?;
        let workspace = workspaces::get(&state.db, &workspace_id).await?;
        let messages = extract_meta_messages(&value);
        let mut processed = 0;
        let mut failures = 0;
        for message in messages {
            let text = match download_whatsapp_media(
                &state,
                &config,
                &workspace.root_path,
                &message,
            )
            .await
            {
                Ok(text) => text,
                Err(error) => {
                    tracing::warn!(error = %error, "WhatsApp media download failed");
                    message.text.clone()
                }
            };
            match process_inbound(
                &state,
                &channel,
                &message.identity,
                &text,
                message.message_id.as_deref(),
                message.name.as_deref(),
                false,
                Value::Null,
                config.workspace_id.as_deref(),
            )
            .await
            {
                Ok(_) => processed += 1,
                Err(error) => {
                    failures += 1;
                    tracing::warn!(identity = %message.identity, error = %error, "WhatsApp message rejected");
                }
            }
        }
        return Ok(Json(json!({ "accepted": processed, "rejected": failures })));
    }

    let expected = state
        .secrets
        .get(WEBHOOK_NAMESPACE, &channel)?
        .ok_or_else(|| {
            AppError::BadRequest(format!(
                "canal {channel} sem token de webhook; gere em Integrações → Canais"
            ))
        })?;
    let provided = headers
        .get(WEBHOOK_HEADER)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if provided != expected {
        return Err(AppError::BadRequest("token do canal inválido".into()));
    }
    let body: InboundBody = serde_json::from_slice(&body)
        .map_err(|error| AppError::BadRequest(format!("payload inválido: {error}")))?;
    let result = process_inbound(
        &state,
        &channel,
        &body.identity,
        &body.text,
        body.message_id.as_deref(),
        body.name.as_deref(),
        body.already_seen,
        body.metadata,
        None,
    )
    .await?;
    Ok(Json(result))
}

#[allow(clippy::too_many_arguments)]
async fn process_inbound(
    state: &Arc<AppState>,
    channel: &str,
    identity: &str,
    text: &str,
    message_id: Option<&str>,
    name: Option<&str>,
    already_seen: bool,
    metadata: Value,
    workspace_override: Option<&str>,
) -> AppResult<Value> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError::BadRequest("mensagem vazia".into()));
    }
    let text = text.chars().take(MAX_TEXT).collect::<String>();
    let identity = {
        let value = identity.trim();
        if value.is_empty() { "unknown" } else { value }
    };

    let workspace_id = crate::api::tasks::resolve_workspace_id(state, workspace_override).await?;
    let route = channels::resolve(&state.db, &workspace_id, channel, identity)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "nenhuma rota para {channel}:{identity}; crie uma em Integrações → Canais"
            ))
        })?;

    let title = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|name| format!("{name} via {channel}"))
        .unwrap_or_else(|| {
            let excerpt = text
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(70)
                .collect::<String>();
            if excerpt.is_empty() {
                format!("Mensagem via {channel}")
            } else {
                excerpt
            }
        });

    let (ticket, created) = tickets::create(
        &state.db,
        CreateTicketInput {
            workspace_id: workspace_id.clone(),
            source: channel.to_string(),
            source_ref: message_id.map(str::to_string),
            title,
            body: text.clone(),
            priority: Some(if route.trust_level == "trusted" { 3 } else { 2 }),
            assigned_agent: Some(route.agent.clone()),
            metadata: json!({
                "identity": identity,
                "name": name,
                "already_seen": already_seen,
                "extra": metadata,
            }),
        },
    )
    .await?;

    let mut task_id: Option<String> = None;
    let mut held = false;
    if created && route.trust_level != "untrusted" {
        let created_task = tasks::create(
            &state.db,
            &workspace_id,
            tasks::CreateTaskInput {
                title: format!("[{channel}] {ticket_title}", ticket_title = ticket.title),
                description: format!("Mensagem recebida de {identity} via {channel}.\n\n{text}"),
                execution_mode: Some("BUILD_DIRECT".into()),
                priority: Some(ticket.priority),
                assigned_agent: Some(route.agent.clone()),
                preferred_model: None,
                preferred_harness: None,
                effort_level: None,
                care_level: Some("standard".into()),
                notify_email: None,
                export_to_gdocs: false,
                tags: vec![
                    format!("channel:{channel}"),
                    format!("trust:{}", route.trust_level),
                ],
                origin: Some("CHANNEL".into()),
                parent_task_id: None,
                dedupe: Some(false),
            },
        )
        .await;
        if let Ok(task) = created_task {
            held = route.permission_profile == "read_only"
                || (route.permission_profile == "default" && route.trust_level != "trusted");
            if held {
                let _ = tasks::set_held(&state.db, &task.id, true).await;
            }
            let _ = tickets::update(
                &state.db,
                &ticket.id,
                tickets::UpdateTicketInput {
                    status: Some("TRIAGED".into()),
                    priority: None,
                    assigned_agent: None,
                    task_id: Some(task.id.clone()),
                    clear_assignee: false,
                },
            )
            .await;
            state.events.publish(OpenDeskWsEvent::TaskUpdated {
                task_id: task.id.clone(),
                workspace_id: workspace_id.clone(),
                stage: "TODO".into(),
                phase_status: Some(if held { "held".into() } else { "queued".into() }),
            });
            task_id = Some(task.id);
        }
    }

    state.events.publish(OpenDeskWsEvent::TicketUpdated {
        ticket_id: ticket.id.clone(),
        workspace_id: workspace_id.clone(),
        source: channel.to_string(),
        status: "OPEN".into(),
    });

    Ok(json!({
        "ticket_id": ticket.id,
        "task_id": task_id,
        "held": held,
        "trust_level": route.trust_level,
        "permission_profile": route.permission_profile,
        "agent": route.agent,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_meta_text_and_interactive_messages() {
        let payload = json!({
            "object": "whatsapp_business_account",
            "entry": [{
                "changes": [{
                    "value": {
                        "contacts": [{ "wa_id": "5511999", "profile": { "name": "Ana" } }],
                        "messages": [
                            { "from": "5511999", "id": "wamid.1", "type": "text", "text": { "body": "Oi" } },
                            { "from": "5511888", "id": "wamid.2", "type": "button", "button": { "text": "Sim" } },
                            { "from": "5511777", "id": "wamid.3", "type": "image", "image": { "id": "media-1", "mime_type": "image/jpeg", "caption": "foto" } }
                        ]
                    }
                }]
            }]
        });
        let messages = extract_meta_messages(&payload);
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].identity, "5511999");
        assert_eq!(messages[0].name.as_deref(), Some("Ana"));
        assert_eq!(messages[1].text, "Sim");
        assert_eq!(messages[2].media_id.as_deref(), Some("media-1"));
        assert_eq!(messages[2].media_mime.as_deref(), Some("image/jpeg"));
        assert_eq!(messages[2].text, "foto");
    }

    #[test]
    fn derives_safe_media_extensions() {
        assert_eq!(media_extension(Some("image/jpeg"), None), "jpg");
        assert_eq!(
            media_extension(Some("application/pdf"), Some("NF.PDF")),
            "pdf"
        );
        assert_eq!(
            media_extension(Some("application/octet-stream"), Some("../../x.sh")),
            "sh"
        );
        assert_eq!(media_extension(None, None), "bin");
    }

    #[test]
    fn verifies_signature() {
        let body = br#"{"hello":"world"}"#;
        let mut mac = Hmac::<Sha256>::new_from_slice(b"secret").expect("mac");
        mac.update(body);
        let encoded = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert!(verify_meta_signature(
            "secret",
            body,
            &format!("sha256={encoded}")
        ));
        assert!(!verify_meta_signature(
            "other",
            body,
            &format!("sha256={encoded}")
        ));
        assert!(!verify_meta_signature("secret", body, "sha1=abc"));
    }
}
