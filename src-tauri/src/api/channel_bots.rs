//! Workspace bots and per-chat context endpoints.

use crate::{
    app_state::AppState,
    db::{channel_bots, settings, workspaces},
    error::{AppError, AppResult},
    integrations::telegram,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub workspace_id: Option<String>,
    pub channel: Option<String>,
    /// Lists bots from every workspace; the UI groups them by workspace.
    #[serde(default)]
    pub all: bool,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListQuery>,
) -> AppResult<Json<Vec<Value>>> {
    let bots = if query.all {
        let mut bots = channel_bots::list_all(&state.db).await?;
        if let Some(channel) = query.channel.as_deref() {
            bots.retain(|bot| bot.channel == channel);
        }
        bots
    } else {
        let workspace_id = match query.workspace_id.filter(|value| !value.trim().is_empty()) {
            Some(value) => value,
            None => settings::active_workspace_id(&state.db)
                .await?
                .ok_or_else(|| AppError::BadRequest("nenhum workspace ativo".into()))?,
        };
        channel_bots::list_bots(&state.db, &workspace_id, query.channel.as_deref()).await?
    };
    let mut values = Vec::with_capacity(bots.len());
    for bot in bots {
        values.push(bot_json(&state, bot).await);
    }
    Ok(Json(values))
}

#[derive(Debug, Deserialize)]
pub struct CreateBotInput {
    pub workspace_id: Option<String>,
    pub channel: String,
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default)]
    pub default_agent: Option<String>,
    #[serde(default)]
    pub config_json: Option<String>,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateBotInput>,
) -> AppResult<Json<Value>> {
    let workspace_id = match input.workspace_id.filter(|value| !value.trim().is_empty()) {
        Some(value) => value,
        None => settings::active_workspace_id(&state.db)
            .await?
            .ok_or_else(|| AppError::BadRequest("nenhum workspace ativo".into()))?,
    };
    workspaces::get(&state.db, &workspace_id).await?;
    let channel = input.channel.trim().to_lowercase();
    let token = normalized_token(&channel, input.token.as_deref())?;
    if let Some(token) = token.as_deref() {
        reject_duplicate_token(&state, None, token).await?;
    }
    let has_token = token.is_some();
    let bot = channel_bots::create_bot(
        &state.db,
        &workspace_id,
        channel_bots::ChannelBotInput {
            channel,
            name: input.name,
            enabled: input.enabled,
            allowlist: input.allowlist,
            default_agent: input.default_agent,
            config_json: input.config_json,
            token: None,
            clear_token: false,
        },
        has_token.then(String::new),
    )
    .await?;
    if let Some(token) = token {
        state.secrets.set("channel_bot", &bot.id, &token)?;
        channel_bots::set_secret_ref(&state.db, &bot.id, Some(&bot.id)).await?;
    }
    let bot = channel_bots::get_bot(&state.db, &bot.id).await?;
    if bot.enabled {
        let _ = state.bot_supervisor.start(state.clone(), bot.clone()).await;
    }
    let bot = channel_bots::get_bot(&state.db, &bot.id).await?;
    Ok(Json(bot_json(&state, bot).await))
}

#[derive(Debug, Deserialize)]
pub struct UpdateBotInput {
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    pub enabled: bool,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub clear_token: bool,
    #[serde(default)]
    pub allowlist: Option<Vec<String>>,
    #[serde(default)]
    pub default_agent: Option<String>,
    #[serde(default)]
    pub config_json: Option<String>,
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<UpdateBotInput>,
) -> AppResult<Json<Value>> {
    let current = channel_bots::get_bot(&state.db, &id).await?;
    let channel = input
        .channel
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&current.channel)
        .to_lowercase();
    let token = normalized_token(&channel, input.token.as_deref())?;
    if let Some(token) = token.as_deref() {
        reject_duplicate_token(&state, Some(&id), token).await?;
    }
    let secret_ref = if input.clear_token {
        state.secrets.delete("channel_bot", &id)?;
        Some(None)
    } else if let Some(token) = token {
        state.secrets.set("channel_bot", &id, &token)?;
        Some(Some(id.clone()))
    } else {
        None
    };
    let bot = channel_bots::update_bot(
        &state.db,
        &id,
        channel_bots::ChannelBotInput {
            channel,
            name: input.name.unwrap_or_default(),
            enabled: input.enabled,
            allowlist: input.allowlist.unwrap_or_default(),
            default_agent: input.default_agent,
            config_json: input.config_json,
            token: None,
            clear_token: false,
        },
        secret_ref,
    )
    .await?;
    state.bot_supervisor.stop(&id).await;
    if bot.enabled {
        let _ = state.bot_supervisor.start(state.clone(), bot.clone()).await;
    } else {
        channel_bots::set_status(&state.db, &id, "stopped", None).await?;
    }
    let bot = channel_bots::get_bot(&state.db, &id).await?;
    Ok(Json(bot_json(&state, bot).await))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    state.bot_supervisor.stop(&id).await;
    let _ = state.secrets.delete("channel_bot", &id);
    channel_bots::delete_bot(&state.db, &id).await?;
    Ok(Json(json!({ "deleted": true })))
}

pub async fn start(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let bot = channel_bots::get_bot(&state.db, &id).await?;
    state.bot_supervisor.start(state.clone(), bot).await?;
    let bot = channel_bots::get_bot(&state.db, &id).await?;
    Ok(Json(bot_json(&state, bot).await))
}

pub async fn stop(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    state.bot_supervisor.stop(&id).await;
    channel_bots::set_status(&state.db, &id, "stopped", None).await?;
    let bot = channel_bots::get_bot(&state.db, &id).await?;
    Ok(Json(bot_json(&state, bot).await))
}

pub async fn test(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let bot = channel_bots::get_bot(&state.db, &id).await?;
    if bot.channel != "telegram" {
        return Err(AppError::BadRequest(
            "o teste de conexão está disponível apenas para Telegram".into(),
        ));
    }
    let token = state
        .secrets
        .get("channel_bot", &id)?
        .ok_or_else(|| AppError::BadRequest("token do bot não configurado".into()))?;
    let info = telegram::get_me(&token).await?;
    let allowlist: Vec<String> = serde_json::from_str(&bot.allowlist_json).unwrap_or_default();
    let targets = telegram::resolve_recipients(&state.db, &allowlist).await;
    if targets.is_empty() {
        return Err(AppError::BadRequest(
            "nenhum chat conhecido na allowlist. Envie /start pelo celular primeiro.".into(),
        ));
    }
    for target in &targets {
        telegram::send_text(&token, target, "OpenDesk conectado a este bot.").await?;
    }
    Ok(Json(json!({ "sent": targets.len(), "bot": info })))
}

pub async fn list_contexts(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<channel_bots::ChannelChatContext>>> {
    channel_bots::get_bot(&state.db, &id).await?;
    Ok(Json(channel_bots::list_contexts(&state.db, &id).await?))
}

pub async fn create_context(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<channel_bots::ChannelChatContextInput>,
) -> AppResult<Json<channel_bots::ChannelChatContext>> {
    let bot = channel_bots::get_bot(&state.db, &id).await?;
    Ok(Json(
        channel_bots::create_context(&state.db, &bot, input).await?,
    ))
}

pub async fn update_context(
    State(state): State<Arc<AppState>>,
    Path((id, ctx_id)): Path<(String, String)>,
    Json(input): Json<channel_bots::ChannelChatContextInput>,
) -> AppResult<Json<channel_bots::ChannelChatContext>> {
    let context = channel_bots::get_context(&state.db, &ctx_id).await?;
    if context.bot_id != id {
        return Err(AppError::NotFound(format!("channel chat context {ctx_id}")));
    }
    Ok(Json(
        channel_bots::update_context(&state.db, &ctx_id, input).await?,
    ))
}

pub async fn delete_context(
    State(state): State<Arc<AppState>>,
    Path((id, ctx_id)): Path<(String, String)>,
) -> AppResult<Json<Value>> {
    let context = channel_bots::get_context(&state.db, &ctx_id).await?;
    if context.bot_id != id {
        return Err(AppError::NotFound(format!("channel chat context {ctx_id}")));
    }
    channel_bots::delete_context(&state.db, &ctx_id).await?;
    Ok(Json(json!({ "deleted": true })))
}

fn normalized_token(channel: &str, token: Option<&str>) -> AppResult<Option<String>> {
    let token = token
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if channel == "telegram"
        && let Some(value) = &token
        && !telegram::valid_token_shape(value)
    {
        return Err(AppError::BadRequest(
            "token do Telegram inválido; use o formato 123456:ABC…".into(),
        ));
    }
    Ok(token)
}

async fn reject_duplicate_token(
    state: &Arc<AppState>,
    current: Option<&str>,
    token: &str,
) -> AppResult<()> {
    for bot in channel_bots::list_all(&state.db).await? {
        if Some(bot.id.as_str()) == current {
            continue;
        }
        if state
            .secrets
            .get("channel_bot", &bot.id)?
            .is_some_and(|stored| stored == token)
        {
            return Err(AppError::Conflict(
                "este token já está em uso por outro bot".into(),
            ));
        }
    }
    Ok(())
}

async fn bot_json(state: &Arc<AppState>, bot: channel_bots::ChannelBot) -> Value {
    let token_configured = state
        .secrets
        .get("channel_bot", &bot.id)
        .ok()
        .flatten()
        .is_some_and(|value| !value.trim().is_empty());
    let running = state.bot_supervisor.is_running(&bot.id).await;
    json!({
        "id": bot.id,
        "workspace_id": bot.workspace_id,
        "channel": bot.channel,
        "name": bot.name,
        "enabled": bot.enabled,
        "token_configured": token_configured,
        "allowlist": serde_json::from_str::<Vec<String>>(&bot.allowlist_json).unwrap_or_default(),
        "default_agent": bot.default_agent,
        "status": if running { "running" } else { bot.status.as_str() },
        "last_error": bot.last_error,
        "last_checked_at": bot.last_checked_at,
        "running": running,
        "config_json": bot.config_json,
        "created_at": bot.created_at,
        "updated_at": bot.updated_at,
    })
}
