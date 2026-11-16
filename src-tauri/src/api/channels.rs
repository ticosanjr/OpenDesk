use crate::{
    app_state::AppState,
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
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

const WHATSAPP_SETTING: &str = "channels.whatsapp";
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
pub struct WhatsAppConfig {
    /// Kept for the future connector; today it only records intent.
    pub enabled: bool,
    /// `structure`, `cloud-api` or `baileys`.
    pub provider: String,
    pub phone_number: String,
    #[serde(default)]
    pub notes: String,
}

impl Default for WhatsAppConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "structure".into(),
            phone_number: String::new(),
            notes: String::new(),
        }
    }
}

pub async fn get_whatsapp(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let config = crate::db::settings::get(&state.db, WHATSAPP_SETTING)
        .await?
        .and_then(|value| serde_json::from_value::<WhatsAppConfig>(value).ok())
        .unwrap_or_default();
    let token_set = state.secrets.get(WEBHOOK_NAMESPACE, "whatsapp")?.is_some();
    Ok(Json(json!({
        "config": config,
        "connected": false,
        "connector": "structure-only",
        "webhook_token_set": token_set,
    })))
}

pub async fn put_whatsapp(
    State(state): State<Arc<AppState>>,
    Json(config): Json<WhatsAppConfig>,
) -> AppResult<Json<Value>> {
    if !["structure", "cloud-api", "baileys"].contains(&config.provider.as_str()) {
        return Err(AppError::BadRequest(
            "provider de WhatsApp inválido; use structure, cloud-api ou baileys".into(),
        ));
    }
    crate::db::settings::set(
        &state.db,
        WHATSAPP_SETTING,
        &serde_json::to_value(&config).map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .await?;
    Ok(Json(json!({
        "config": config,
        "connected": false,
        "connector": "structure-only",
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
                "structure-only".into()
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

/// Universal inbound webhook. Requires the channel token created through
/// `/api/channels/{channel}/webhook-token`. Trust decides whether a task is
/// created and whether it starts held for human review.
pub async fn inbound(
    State(state): State<Arc<AppState>>,
    Path(channel): Path<String>,
    headers: HeaderMap,
    Json(body): Json<InboundBody>,
) -> AppResult<Json<Value>> {
    let channel = channels::validate_channel(&channel)?;
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

    let text = body.text.trim();
    if text.is_empty() {
        return Err(AppError::BadRequest("mensagem vazia".into()));
    }
    let text = text.chars().take(MAX_TEXT).collect::<String>();
    let identity = {
        let value = body.identity.trim();
        if value.is_empty() { "unknown" } else { value }
    };

    // Without an explicit workspace in the payload, use the active/default one.
    let workspace_id = crate::api::tasks::resolve_workspace_id(&state, None).await?;
    let route = channels::resolve(&state.db, &workspace_id, &channel, identity)
        .await?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "nenhuma rota para {channel}:{identity}; crie uma em Integrações → Canais"
            ))
        })?;

    let title = body
        .name
        .as_deref()
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
            source: channel.clone(),
            source_ref: body.message_id.clone(),
            title,
            body: text.clone(),
            priority: Some(if route.trust_level == "trusted" { 3 } else { 2 }),
            assigned_agent: Some(route.agent.clone()),
            metadata: json!({
                "identity": identity,
                "name": body.name,
                "already_seen": body.already_seen,
                "extra": body.metadata,
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
        source: channel.clone(),
        status: "OPEN".into(),
    });

    Ok(Json(json!({
        "ticket_id": ticket.id,
        "task_id": task_id,
        "held": held,
        "trust_level": route.trust_level,
        "permission_profile": route.permission_profile,
        "agent": route.agent,
    })))
}
