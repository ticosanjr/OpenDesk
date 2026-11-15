use crate::error::{AppError, AppResult};
use rand::RngCore;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AuthSession {
    pub id: String,
    pub token_hash: String,
    pub label: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub expires_at: String,
    pub revoked_at: Option<String>,
}

const COLUMNS: &str = "id, token_hash, label, created_at, last_used_at, expires_at, revoked_at";

fn hash(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub async fn create(
    pool: &SqlitePool,
    label: &str,
    ttl_days: i64,
) -> AppResult<(String, AuthSession)> {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let token = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let id = Uuid::now_v7().to_string();
    let ttl = ttl_days.clamp(1, 365);
    sqlx::query(
        "INSERT INTO auth_sessions (id, token_hash, label, expires_at)
         VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ','now', ?))",
    )
    .bind(&id)
    .bind(hash(&token))
    .bind(label.trim())
    .bind(format!("+{ttl} days"))
    .execute(pool)
    .await?;
    let session = get(pool, &id).await?;
    Ok((token, session))
}

pub async fn validate(pool: &SqlitePool, token: &str) -> AppResult<bool> {
    if token.trim().is_empty() {
        return Ok(false);
    }
    let hash = hash(token);
    let found: Option<String> = sqlx::query_scalar(
        "SELECT id FROM auth_sessions
         WHERE token_hash = ? AND revoked_at IS NULL
           AND expires_at > strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await?;
    let Some(id) = found else {
        return Ok(false);
    };
    let _ = sqlx::query(
        "UPDATE auth_sessions SET last_used_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(&id)
    .execute(pool)
    .await;
    Ok(true)
}

pub async fn revoke(pool: &SqlitePool, token: &str) -> AppResult<bool> {
    let result = sqlx::query(
        "UPDATE auth_sessions SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE token_hash = ? AND revoked_at IS NULL",
    )
    .bind(hash(token))
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<AuthSession>> {
    let query = format!("SELECT {COLUMNS} FROM auth_sessions ORDER BY created_at DESC LIMIT 100");
    Ok(sqlx::query_as::<_, AuthSession>(&query)
        .fetch_all(pool)
        .await?)
}

async fn get(pool: &SqlitePool, id: &str) -> AppResult<AuthSession> {
    let query = format!("SELECT {COLUMNS} FROM auth_sessions WHERE id = ?");
    sqlx::query_as::<_, AuthSession>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("auth session {id}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn creates_validates_and_revokes_sessions() {
        let ctx = crate::test_support::context().await;
        let (token, session) = create(&ctx.state.db, "pwa", 30).await.expect("session");
        assert_eq!(session.label, "pwa");
        assert!(validate(&ctx.state.db, &token).await.expect("validate"));
        assert!(!validate(&ctx.state.db, "wrong").await.expect("wrong"));
        assert!(revoke(&ctx.state.db, &token).await.expect("revoke"));
        assert!(!validate(&ctx.state.db, &token).await.expect("revoked"));
    }
}
