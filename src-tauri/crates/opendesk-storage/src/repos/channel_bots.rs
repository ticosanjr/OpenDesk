//! Bots de chat por workspace e contextos isolados por conversa/grupo.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChannelBot {
    pub id: String,
    pub workspace_id: String,
    pub channel: String,
    pub name: String,
    pub enabled: bool,
    pub secret_ref: Option<String>,
    pub config_json: String,
    pub allowlist_json: String,
    pub default_agent: String,
    pub status: String,
    pub last_error: Option<String>,
    pub last_checked_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChannelChatContext {
    pub id: String,
    pub bot_id: String,
    pub workspace_id: String,
    pub channel: String,
    pub external_chat_id: String,
    pub chat_type: String,
    pub title: String,
    pub system_context: String,
    pub agent_override: Option<String>,
    pub memory_scope: String,
    pub opencode_session_id: Option<String>,
    pub enabled: bool,
    pub last_message_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ChannelBotInput {
    pub channel: String,
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub clear_token: bool,
    #[serde(default)]
    pub config_json: Option<String>,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default)]
    pub default_agent: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ChannelChatContextInput {
    #[serde(default)]
    pub external_chat_id: Option<String>,
    #[serde(default)]
    pub chat_type: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub system_context: Option<String>,
    #[serde(default)]
    pub agent_override: Option<Option<String>>,
    #[serde(default)]
    pub memory_scope: Option<String>,
    #[serde(default)]
    pub opencode_session_id: Option<Option<String>>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

const BOT_COLUMNS: &str = "id, workspace_id, channel, name, enabled, secret_ref, config_json, \
    allowlist_json, default_agent, status, last_error, last_checked_at, created_at, updated_at";
const CONTEXT_COLUMNS: &str = "id, bot_id, workspace_id, channel, external_chat_id, chat_type, \
    title, system_context, agent_override, memory_scope, opencode_session_id, enabled, \
    last_message_at, created_at, updated_at";

pub async fn list_bots(
    pool: &SqlitePool,
    workspace_id: &str,
    channel: Option<&str>,
) -> AppResult<Vec<ChannelBot>> {
    let query = format!(
        "SELECT {BOT_COLUMNS} FROM channel_bots WHERE workspace_id = ?
         AND (? IS NULL OR channel = ?) ORDER BY channel, name"
    );
    Ok(sqlx::query_as::<_, ChannelBot>(&query)
        .bind(workspace_id)
        .bind(channel)
        .bind(channel)
        .fetch_all(pool)
        .await?)
}

pub async fn all_enabled(pool: &SqlitePool) -> AppResult<Vec<ChannelBot>> {
    let query = format!(
        "SELECT {BOT_COLUMNS} FROM channel_bots WHERE enabled = 1 ORDER BY workspace_id, channel, name"
    );
    Ok(sqlx::query_as::<_, ChannelBot>(&query)
        .fetch_all(pool)
        .await?)
}

pub async fn get_bot(pool: &SqlitePool, id: &str) -> AppResult<ChannelBot> {
    let query = format!("SELECT {BOT_COLUMNS} FROM channel_bots WHERE id = ?");
    sqlx::query_as::<_, ChannelBot>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("channel bot {id}")))
}

pub async fn create_bot(
    pool: &SqlitePool,
    workspace_id: &str,
    input: ChannelBotInput,
    secret_ref: Option<String>,
) -> AppResult<ChannelBot> {
    let id = Uuid::now_v7().to_string();
    let channel = input.channel.trim().to_lowercase();
    if !matches!(
        channel.as_str(),
        "telegram" | "signal" | "whatsapp" | "matrix"
    ) {
        return Err(AppError::BadRequest(format!("canal inválido: {channel}")));
    }
    if input.name.trim().is_empty() {
        return Err(AppError::BadRequest("nome do bot é obrigatório".into()));
    }
    let config = input
        .config_json
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("{}");
    serde_json::from_str::<serde_json::Value>(config)
        .map_err(|error| AppError::BadRequest(format!("config_json inválido: {error}")))?;
    let allowlist = serde_json::to_string(&input.allowlist)
        .map_err(|error| AppError::Internal(format!("allowlist inválida: {error}")))?;
    let agent = input
        .default_agent
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("build");
    sqlx::query(
        "INSERT INTO channel_bots
            (id, workspace_id, channel, name, enabled, secret_ref, config_json,
             allowlist_json, default_agent)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(&channel)
    .bind(input.name.trim())
    .bind(input.enabled)
    .bind(secret_ref.as_deref())
    .bind(config)
    .bind(&allowlist)
    .bind(agent)
    .execute(pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            AppError::Conflict("já existe um bot com esse nome neste workspace/canal".into())
        }
        other => AppError::Db(other),
    })?;
    get_bot(pool, &id).await
}

pub async fn update_bot(
    pool: &SqlitePool,
    id: &str,
    input: ChannelBotInput,
    secret_ref: Option<Option<String>>,
) -> AppResult<ChannelBot> {
    let current = get_bot(pool, id).await?;
    let name = input.name.trim();
    let name = if name.is_empty() {
        current.name
    } else {
        name.to_string()
    };
    let channel = if input.channel.trim().is_empty() {
        current.channel
    } else {
        input.channel.trim().to_lowercase()
    };
    let config = input
        .config_json
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&current.config_json)
        .to_string();
    serde_json::from_str::<serde_json::Value>(&config)
        .map_err(|error| AppError::BadRequest(format!("config_json inválido: {error}")))?;
    let allowlist = if input.allowlist.is_empty() {
        current.allowlist_json
    } else {
        serde_json::to_string(&input.allowlist)
            .map_err(|error| AppError::Internal(format!("allowlist inválida: {error}")))?
    };
    let agent = input
        .default_agent
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&current.default_agent)
        .to_string();
    let secret_ref = secret_ref.unwrap_or(current.secret_ref);
    sqlx::query(
        "UPDATE channel_bots SET name = ?, channel = ?, enabled = ?, secret_ref = ?,
            config_json = ?, allowlist_json = ?, default_agent = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(&name)
    .bind(&channel)
    .bind(input.enabled)
    .bind(secret_ref.as_deref())
    .bind(&config)
    .bind(&allowlist)
    .bind(&agent)
    .bind(id)
    .execute(pool)
    .await?;
    get_bot(pool, id).await
}

pub async fn delete_bot(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM channel_bots WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("channel bot {id}")));
    }
    Ok(())
}

pub async fn set_status(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    last_error: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE channel_bots SET status = ?, last_error = ?,
            last_checked_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(status)
    .bind(last_error)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_contexts(pool: &SqlitePool, bot_id: &str) -> AppResult<Vec<ChannelChatContext>> {
    let query = format!(
        "SELECT {CONTEXT_COLUMNS} FROM channel_chat_contexts WHERE bot_id = ? ORDER BY title"
    );
    Ok(sqlx::query_as::<_, ChannelChatContext>(&query)
        .bind(bot_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get_context(pool: &SqlitePool, id: &str) -> AppResult<ChannelChatContext> {
    let query = format!("SELECT {CONTEXT_COLUMNS} FROM channel_chat_contexts WHERE id = ?");
    sqlx::query_as::<_, ChannelChatContext>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("channel chat context {id}")))
}

pub async fn find_context(
    pool: &SqlitePool,
    bot_id: &str,
    external_chat_id: &str,
) -> AppResult<Option<ChannelChatContext>> {
    let query = format!(
        "SELECT {CONTEXT_COLUMNS} FROM channel_chat_contexts WHERE bot_id = ? AND external_chat_id = ?"
    );
    Ok(sqlx::query_as::<_, ChannelChatContext>(&query)
        .bind(bot_id)
        .bind(external_chat_id)
        .fetch_optional(pool)
        .await?)
}

pub async fn upsert_seen_context(
    pool: &SqlitePool,
    bot: &ChannelBot,
    external_chat_id: &str,
    chat_type: &str,
    title: &str,
) -> AppResult<ChannelChatContext> {
    if let Some(current) = find_context(pool, &bot.id, external_chat_id).await? {
        sqlx::query(
            "UPDATE channel_chat_contexts SET title = ?, chat_type = ?,
                last_message_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
        )
        .bind(title)
        .bind(chat_type)
        .bind(&current.id)
        .execute(pool)
        .await?;
        return get_context(pool, &current.id).await;
    }
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO channel_chat_contexts
            (id, bot_id, workspace_id, channel, external_chat_id, chat_type, title, last_message_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
    )
    .bind(&id)
    .bind(&bot.id)
    .bind(&bot.workspace_id)
    .bind(&bot.channel)
    .bind(external_chat_id)
    .bind(chat_type)
    .bind(title)
    .execute(pool)
    .await?;
    get_context(pool, &id).await
}

pub async fn create_context(
    pool: &SqlitePool,
    bot: &ChannelBot,
    input: ChannelChatContextInput,
) -> AppResult<ChannelChatContext> {
    let external = input
        .external_chat_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::BadRequest("external_chat_id é obrigatório".into()))?;
    if find_context(pool, &bot.id, external).await?.is_some() {
        return Err(AppError::Conflict(
            "contexto já existe para este chat".into(),
        ));
    }
    let id = Uuid::now_v7().to_string();
    let title = input.title.as_deref().unwrap_or(external);
    sqlx::query(
        "INSERT INTO channel_chat_contexts
            (id, bot_id, workspace_id, channel, external_chat_id, chat_type, title,
             system_context, memory_scope, agent_override, opencode_session_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&bot.id)
    .bind(&bot.workspace_id)
    .bind(&bot.channel)
    .bind(external)
    .bind(input.chat_type.as_deref().unwrap_or("group"))
    .bind(title)
    .bind(input.system_context.as_deref().unwrap_or(""))
    .bind(input.memory_scope.as_deref().unwrap_or("chat"))
    .bind(input.agent_override.clone().flatten())
    .bind(input.opencode_session_id.clone().flatten())
    .execute(pool)
    .await?;
    get_context(pool, &id).await
}

pub async fn update_context(
    pool: &SqlitePool,
    id: &str,
    input: ChannelChatContextInput,
) -> AppResult<ChannelChatContext> {
    let current = get_context(pool, id).await?;
    let scope = input
        .memory_scope
        .as_deref()
        .unwrap_or(&current.memory_scope);
    if !matches!(scope, "chat" | "normal" | "both" | "none") {
        return Err(AppError::BadRequest(format!(
            "memory_scope inválido: {scope}"
        )));
    }
    sqlx::query(
        "UPDATE channel_chat_contexts SET title = ?, chat_type = ?, system_context = ?,
            agent_override = ?, memory_scope = ?, opencode_session_id = ?, enabled = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(input.title.as_deref().unwrap_or(&current.title))
    .bind(input.chat_type.as_deref().unwrap_or(&current.chat_type))
    .bind(
        input
            .system_context
            .as_deref()
            .unwrap_or(&current.system_context),
    )
    .bind(input.agent_override.unwrap_or(current.agent_override))
    .bind(scope)
    .bind(
        input
            .opencode_session_id
            .unwrap_or(current.opencode_session_id),
    )
    .bind(input.enabled.unwrap_or(current.enabled))
    .bind(id)
    .execute(pool)
    .await?;
    get_context(pool, id).await
}

pub async fn delete_context(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM channel_chat_contexts WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("channel chat context {id}")));
    }
    Ok(())
}

pub async fn count_bots(pool: &SqlitePool) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM channel_bots")
            .fetch_one(pool)
            .await?,
    )
}

pub async fn set_secret_ref(
    pool: &SqlitePool,
    id: &str,
    secret_ref: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE channel_bots SET secret_ref = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(secret_ref)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_all(pool: &SqlitePool) -> AppResult<Vec<ChannelBot>> {
    let query =
        format!("SELECT {BOT_COLUMNS} FROM channel_bots ORDER BY workspace_id, channel, name");
    Ok(sqlx::query_as::<_, ChannelBot>(&query)
        .fetch_all(pool)
        .await?)
}

pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> AppResult<()> {
    sqlx::query(
        "UPDATE channel_bots SET enabled = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(enabled)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}
