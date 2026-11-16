//! Inbound channel routes and trust.
//!
//! A route answers two questions for an inbound message: which agent handles it
//! and how much it is allowed to do. There is no real WhatsApp connector yet;
//! the route/config model is shared by every channel (Telegram, Signal, email,
//! webhook, WhatsApp when it lands).

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

pub const CHANNELS: &[&str] = &["whatsapp", "telegram", "signal", "email", "web", "api"];
pub const TRUST_LEVELS: &[&str] = &["trusted", "internal", "untrusted"];
pub const PERMISSION_PROFILES: &[&str] = &["default", "read_only", "full"];

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ChannelRoute {
    pub id: String,
    pub workspace_id: String,
    pub channel: String,
    pub identity: String,
    pub agent: String,
    pub permission_profile: String,
    pub trust_level: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertRouteInput {
    pub channel: String,
    /// Conversation/account id, or `*` for the channel default.
    #[serde(default = "wildcard")]
    pub identity: String,
    pub agent: String,
    #[serde(default = "default_profile")]
    pub permission_profile: String,
    #[serde(default = "default_trust")]
    pub trust_level: String,
}

fn wildcard() -> String {
    "*".into()
}

fn default_profile() -> String {
    "default".into()
}

fn default_trust() -> String {
    "untrusted".into()
}

const COLUMNS: &str = "id, workspace_id, channel, identity, agent, permission_profile, trust_level, created_at, updated_at";

pub async fn list(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<ChannelRoute>> {
    let query = format!(
        "SELECT {COLUMNS} FROM channel_routes WHERE workspace_id = ?
         ORDER BY channel, identity"
    );
    Ok(sqlx::query_as::<_, ChannelRoute>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<ChannelRoute> {
    let query = format!("SELECT {COLUMNS} FROM channel_routes WHERE id = ?");
    sqlx::query_as::<_, ChannelRoute>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("channel route {id}")))
}

pub async fn upsert(
    pool: &SqlitePool,
    workspace_id: &str,
    input: UpsertRouteInput,
) -> AppResult<ChannelRoute> {
    let channel = validate_channel(&input.channel)?;
    let trust = validate_trust(&input.trust_level)?;
    let profile = validate_profile(&input.permission_profile)?;
    let identity = input.identity.trim();
    let identity = if identity.is_empty() { "*" } else { identity };
    let agent = input.agent.trim();
    if agent.is_empty() {
        return Err(AppError::BadRequest("rota precisa de um agente".into()));
    }

    if let Some(existing) = sqlx::query_scalar::<_, String>(
        "SELECT id FROM channel_routes WHERE channel = ? AND identity = ?",
    )
    .bind(&channel)
    .bind(identity)
    .fetch_optional(pool)
    .await?
    {
        sqlx::query(
            "UPDATE channel_routes SET workspace_id = ?, agent = ?, permission_profile = ?,
                    trust_level = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?",
        )
        .bind(workspace_id)
        .bind(agent)
        .bind(&profile)
        .bind(&trust)
        .bind(&existing)
        .execute(pool)
        .await?;
        return get(pool, &existing).await;
    }

    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO channel_routes (id, workspace_id, channel, identity, agent, permission_profile, trust_level)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(&channel)
    .bind(identity)
    .bind(agent)
    .bind(&profile)
    .bind(&trust)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<bool> {
    let result = sqlx::query("DELETE FROM channel_routes WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

/// Exact identity wins over the channel wildcard.
pub async fn resolve(
    pool: &SqlitePool,
    workspace_id: &str,
    channel: &str,
    identity: &str,
) -> AppResult<Option<ChannelRoute>> {
    let query = format!(
        "SELECT {COLUMNS} FROM channel_routes
         WHERE workspace_id = ? AND channel = ? AND identity IN (?, '*')
         ORDER BY CASE WHEN identity = ? THEN 0 ELSE 1 END
         LIMIT 1"
    );
    Ok(sqlx::query_as::<_, ChannelRoute>(&query)
        .bind(workspace_id)
        .bind(channel)
        .bind(identity)
        .bind(identity)
        .fetch_optional(pool)
        .await?)
}

pub fn validate_channel(value: &str) -> AppResult<String> {
    let value = value.trim().to_lowercase();
    if CHANNELS.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(AppError::BadRequest(format!(
            "canal inválido '{value}'; use {}",
            CHANNELS.join(", ")
        )))
    }
}

fn validate_trust(value: &str) -> AppResult<String> {
    let value = value.trim().to_lowercase();
    if TRUST_LEVELS.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(AppError::BadRequest(format!(
            "trust inválido '{value}'; use {}",
            TRUST_LEVELS.join(", ")
        )))
    }
}

fn validate_profile(value: &str) -> AppResult<String> {
    let value = value.trim().to_lowercase();
    if PERMISSION_PROFILES.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(AppError::BadRequest(format!(
            "perfil inválido '{value}'; use {}",
            PERMISSION_PROFILES.join(", ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolves_exact_before_wildcard() {
        let ctx = crate::test_support::context().await;
        upsert(
            &ctx.state.db,
            &ctx.workspace_id,
            UpsertRouteInput {
                channel: "telegram".into(),
                identity: "*".into(),
                agent: "build".into(),
                permission_profile: "read_only".into(),
                trust_level: "untrusted".into(),
            },
        )
        .await
        .expect("wildcard");
        let route = resolve(&ctx.state.db, &ctx.workspace_id, "telegram", "42")
            .await
            .expect("resolve")
            .expect("route");
        assert_eq!(route.agent, "build");

        upsert(
            &ctx.state.db,
            &ctx.workspace_id,
            UpsertRouteInput {
                channel: "telegram".into(),
                identity: "42".into(),
                agent: "plan".into(),
                permission_profile: "default".into(),
                trust_level: "trusted".into(),
            },
        )
        .await
        .expect("exact");
        let route = resolve(&ctx.state.db, &ctx.workspace_id, "telegram", "42")
            .await
            .expect("resolve")
            .expect("route");
        assert_eq!(route.agent, "plan");
        assert_eq!(route.trust_level, "trusted");
    }

    #[tokio::test]
    async fn rejects_invalid_routes() {
        let ctx = crate::test_support::context().await;
        let error = upsert(
            &ctx.state.db,
            &ctx.workspace_id,
            UpsertRouteInput {
                channel: "carrier-pigeon".into(),
                identity: "1".into(),
                agent: "build".into(),
                permission_profile: "default".into(),
                trust_level: "trusted".into(),
            },
        )
        .await
        .expect_err("invalid channel");
        assert!(matches!(error, AppError::BadRequest(_)));
    }
}
