use super::{MAX_TEXT, WhatsAppConfig};
use crate::{
    app_state::AppState,
    db::workspaces,
    db::{
        channels, tasks,
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
use opendesk_channels::whatsapp::{
    self, WEBHOOK_HEADER, WEBHOOK_NAMESPACE, WHATSAPP_SECRET_NAMESPACE,
};
use rand::RngCore;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

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

async fn download_whatsapp_media(
    state: &Arc<AppState>,
    config: &WhatsAppConfig,
    workspace_root: &str,
    message: &whatsapp::MetaMessage,
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
    let extension = whatsapp::media_extension(mime, message.media_name.as_deref());
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
        if !whatsapp::verify_signature(&secret, &body, signature) {
            return Err(AppError::BadRequest(
                "assinatura do webhook inválida".into(),
            ));
        }
        let config = whatsapp::load_config(&state.db).await?;
        let workspace_id =
            crate::api::tasks::resolve_workspace_id(&state, config.workspace_id.as_deref()).await?;
        let workspace = workspaces::get(&state.db, &workspace_id).await?;
        let messages = whatsapp::extract_meta_messages(&value);
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
