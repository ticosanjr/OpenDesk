//! Multi-node control plane.
//!
//! Nodes are remote OpenDesk workers that pull queued jobs, heartbeat the lease
//! and report completion. The central queue remains the source of truth; the
//! worker is authenticated by a hashed per-node token.

use crate::error::{AppError, AppResult};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct Worker {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub capabilities_json: String,
    pub status: String,
    #[allow(dead_code)]
    pub token_hash: Option<String>,
    pub last_seen_at: Option<String>,
    pub created_at: String,
    #[allow(dead_code)]
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NodeView {
    pub id: String,
    pub name: String,
    pub endpoint: String,
    pub capabilities: Vec<String>,
    pub status: String,
    pub online: bool,
    pub last_seen_at: Option<String>,
    pub created_at: String,
}

impl Worker {
    pub fn view(&self) -> NodeView {
        let online = self
            .last_seen_at
            .as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
            .map(|value| {
                chrono::Utc::now()
                    .signed_duration_since(value.with_timezone(&chrono::Utc))
                    .num_seconds()
                    < 90
            })
            .unwrap_or(false);
        NodeView {
            id: self.id.clone(),
            name: self.name.clone(),
            endpoint: self.endpoint.clone(),
            capabilities: serde_json::from_str(&self.capabilities_json).unwrap_or_default(),
            status: self.status.clone(),
            online,
            last_seen_at: self.last_seen_at.clone(),
            created_at: self.created_at.clone(),
        }
    }
}

const COLUMNS: &str = "id, name, endpoint, capabilities_json, status, token_hash, \
    last_seen_at, created_at, updated_at";

fn hash(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<NodeView>> {
    let query = format!("SELECT {COLUMNS} FROM workers ORDER BY created_at DESC");
    Ok(sqlx::query_as::<_, Worker>(&query)
        .fetch_all(pool)
        .await?
        .iter()
        .map(Worker::view)
        .collect())
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Worker> {
    let query = format!("SELECT {COLUMNS} FROM workers WHERE id = ?");
    sqlx::query_as::<_, Worker>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("node {id}")))
}

pub async fn create(
    pool: &SqlitePool,
    name: &str,
    endpoint: &str,
    capabilities: Vec<String>,
) -> AppResult<(String, NodeView)> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("nome do nó é obrigatório".into()));
    }
    let endpoint = endpoint.trim();
    if !endpoint.is_empty() && !endpoint.starts_with("http://") && !endpoint.starts_with("https://")
    {
        return Err(AppError::BadRequest(
            "endpoint do nó precisa ser http(s)".into(),
        ));
    }
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let token = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO workers (id, name, endpoint, capabilities_json, status, token_hash)
         VALUES (?, ?, ?, ?, 'OFFLINE', ?)",
    )
    .bind(&id)
    .bind(name)
    .bind(endpoint)
    .bind(serde_json::to_string(&capabilities).unwrap_or_else(|_| "[]".into()))
    .bind(hash(&token))
    .execute(pool)
    .await?;
    Ok((token, get(pool, &id).await?.view()))
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<bool> {
    let result = sqlx::query("DELETE FROM workers WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct HeartbeatInput {
    #[serde(default)]
    pub capabilities: Option<Vec<String>>,
    #[serde(default)]
    pub status: Option<String>,
}

pub async fn heartbeat(
    pool: &SqlitePool,
    token: &str,
    input: HeartbeatInput,
) -> AppResult<NodeView> {
    let worker = authenticate(pool, token)
        .await?
        .ok_or_else(|| AppError::BadRequest("token de nó inválido".into()))?;
    let status = match input.status.as_deref() {
        Some("BUSY") => "BUSY",
        Some("DRAINING") => "DRAINING",
        Some("OFFLINE") => "OFFLINE",
        _ => "ONLINE",
    };
    let capabilities = input
        .capabilities
        .map(|list| serde_json::to_string(&list).unwrap_or_else(|_| "[]".into()));
    sqlx::query(
        "UPDATE workers SET status = ?, last_seen_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
                capabilities_json = COALESCE(?, capabilities_json),
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(status)
    .bind(capabilities)
    .bind(&worker.id)
    .execute(pool)
    .await?;
    Ok(get(pool, &worker.id).await?.view())
}

/// Validates a node token without touching heartbeat state.
pub async fn authenticate(pool: &SqlitePool, token: &str) -> AppResult<Option<Worker>> {
    if token.trim().is_empty() {
        return Ok(None);
    }
    let query = format!("SELECT {COLUMNS} FROM workers WHERE token_hash = ?");
    Ok(sqlx::query_as::<_, Worker>(&query)
        .bind(hash(token))
        .fetch_optional(pool)
        .await?)
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct UpdateNodeInput {
    pub name: Option<String>,
    pub endpoint: Option<String>,
    pub capabilities: Option<Vec<String>>,
    pub status: Option<String>,
}

pub async fn update(pool: &SqlitePool, id: &str, input: UpdateNodeInput) -> AppResult<NodeView> {
    let current = get(pool, id).await?;
    let status = match input.status {
        Some(value) => {
            let value = value.trim().to_uppercase();
            if !matches!(value.as_str(), "ONLINE" | "BUSY" | "OFFLINE" | "DRAINING") {
                return Err(AppError::BadRequest(format!(
                    "status de nó inválido '{value}'"
                )));
            }
            value
        }
        None => current.status,
    };
    sqlx::query(
        "UPDATE workers SET name = ?, endpoint = ?, capabilities_json = ?, status = ?,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(input.name.unwrap_or(current.name))
    .bind(input.endpoint.unwrap_or(current.endpoint))
    .bind(
        input
            .capabilities
            .map(|list| serde_json::to_string(&list).unwrap_or_else(|_| "[]".into()))
            .unwrap_or(current.capabilities_json),
    )
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(get(pool, id).await?.view())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn registers_heartbeats_and_authenticates_nodes() {
        let ctx = crate::test_support::context().await;
        let (token, node) = create(
            &ctx.state.db,
            "worker-1",
            "http://10.0.0.5:4096",
            vec!["opencode".into()],
        )
        .await
        .expect("create");
        assert_eq!(node.status, "OFFLINE");
        assert!(!node.online);

        let worker = authenticate(&ctx.state.db, &token)
            .await
            .expect("auth")
            .expect("worker");
        assert_eq!(worker.name, "worker-1");
        assert!(
            authenticate(&ctx.state.db, "nope")
                .await
                .expect("auth")
                .is_none()
        );

        let beat = heartbeat(
            &ctx.state.db,
            &token,
            HeartbeatInput {
                capabilities: Some(vec!["opencode".into(), "codex".into()]),
                status: Some("BUSY".into()),
            },
        )
        .await
        .expect("heartbeat");
        assert_eq!(beat.status, "BUSY");
        assert!(beat.online);
        assert_eq!(beat.capabilities.len(), 2);

        assert!(delete(&ctx.state.db, &beat.id).await.expect("delete"));
        assert!(list(&ctx.state.db).await.expect("list").is_empty());
    }
}
