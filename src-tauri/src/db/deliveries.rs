use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Delivery {
    pub id: String,
    pub workspace_id: String,
    pub artifact_id: Option<String>,
    pub channel: String,
    pub idempotency_key: String,
    pub status: String,
    pub attempts: i64,
    pub last_error: Option<String>,
    pub external_id: Option<String>,
    pub external_url: Option<String>,
    pub next_retry_at: Option<String>,
    pub max_attempts: i64,
    pub payload_json: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

const COLUMNS: &str = "id, workspace_id, artifact_id, channel, idempotency_key, status, \
    attempts, last_error, external_id, external_url, next_retry_at, max_attempts, payload_json, \
    created_at, updated_at";

pub async fn create_pending(
    pool: &SqlitePool,
    workspace_id: &str,
    artifact_id: Option<&str>,
    channel: &str,
    idempotency_key: &str,
) -> AppResult<Option<Delivery>> {
    let id = Uuid::now_v7().to_string();
    let result = sqlx::query(
        "INSERT OR IGNORE INTO integration_deliveries (
            id, workspace_id, artifact_id, channel, idempotency_key, status
         ) VALUES (?, ?, ?, ?, ?, 'PENDING')",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(artifact_id)
    .bind(channel)
    .bind(idempotency_key)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }

    get_by_key(pool, idempotency_key).await
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<Delivery> {
    let query = format!("SELECT {COLUMNS} FROM integration_deliveries WHERE id = ?");
    sqlx::query_as::<_, Delivery>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| crate::error::AppError::NotFound(format!("delivery {id}")))
}

pub async fn get_by_key(pool: &SqlitePool, key: &str) -> AppResult<Option<Delivery>> {
    let query = format!("SELECT {COLUMNS} FROM integration_deliveries WHERE idempotency_key = ?");
    Ok(sqlx::query_as::<_, Delivery>(&query)
        .bind(key)
        .fetch_optional(pool)
        .await?)
}

pub async fn mark_success(
    pool: &SqlitePool,
    id: &str,
    external_id: Option<&str>,
    external_url: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE integration_deliveries SET status = 'SUCCESS',
            attempts = attempts + 1, external_id = COALESCE(?, external_id),
            external_url = COALESCE(?, external_url),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(external_id)
    .bind(external_url)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_failed(pool: &SqlitePool, id: &str, error: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE integration_deliveries SET status = 'FAILED',
            attempts = attempts + 1, last_error = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(error)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_failed_retryable(
    pool: &SqlitePool,
    id: &str,
    error: &str,
    payload: &str,
    delay_minutes: i64,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE integration_deliveries SET status = 'FAILED',
            attempts = attempts + 1, last_error = ?, payload_json = ?,
            next_retry_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(error)
    .bind(payload)
    .bind(format!("+{delay_minutes} minutes"))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn due_retries(pool: &SqlitePool, limit: i64) -> AppResult<Vec<Delivery>> {
    let query = format!(
        "SELECT {COLUMNS} FROM integration_deliveries
         WHERE status = 'FAILED' AND payload_json IS NOT NULL
           AND attempts < max_attempts
           AND next_retry_at IS NOT NULL
           AND next_retry_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         ORDER BY next_retry_at ASC LIMIT ?"
    );
    Ok(sqlx::query_as::<_, Delivery>(&query)
        .bind(limit.clamp(1, 50))
        .fetch_all(pool)
        .await?)
}

pub async fn reschedule(
    pool: &SqlitePool,
    id: &str,
    error: &str,
    delay_minutes: i64,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE integration_deliveries SET attempts = attempts + 1, last_error = ?,
            next_retry_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(error)
    .bind(format!("+{delay_minutes} minutes"))
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_skipped(pool: &SqlitePool, id: &str, reason: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE integration_deliveries SET status = 'SKIPPED', last_error = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(reason)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list(pool: &SqlitePool, workspace_id: &str, limit: i64) -> AppResult<Vec<Delivery>> {
    let query = format!(
        "SELECT {COLUMNS} FROM integration_deliveries
         WHERE workspace_id = ? ORDER BY created_at DESC LIMIT ?"
    );
    Ok(sqlx::query_as::<_, Delivery>(&query)
        .bind(workspace_id)
        .bind(limit.clamp(1, 200))
        .fetch_all(pool)
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("deliveries.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        (temp, pool)
    }

    #[tokio::test]
    async fn failed_deliveries_are_scheduled_for_retry() {
        let (_temp, pool) = setup().await;
        let delivery = create_pending(&pool, "ws-1", None, "SMTP", "smtp:run-1:a@b.com")
            .await
            .expect("create")
            .expect("created");
        mark_failed_retryable(&pool, &delivery.id, "timeout", "{}", 0)
            .await
            .expect("fail");
        let due = due_retries(&pool, 10).await.expect("due");
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].attempts, 1);
        reschedule(&pool, &delivery.id, "again", 0)
            .await
            .expect("reschedule");
        assert_eq!(due_retries(&pool, 10).await.expect("due 2").len(), 1);
    }

    #[tokio::test]
    async fn idempotency_key_prevents_duplicate_delivery() {
        let (_temp, pool) = setup().await;
        let first = create_pending(&pool, "ws-1", None, "SMTP", "smtp:run-1:a@b.com")
            .await
            .expect("first")
            .expect("created");
        assert_eq!(first.status, "PENDING");

        let duplicate = create_pending(&pool, "ws-1", None, "SMTP", "smtp:run-1:a@b.com")
            .await
            .expect("duplicate");
        assert!(duplicate.is_none());

        mark_success(&pool, &first.id, Some("msg-1"), None)
            .await
            .expect("success");
        let stored = get_by_key(&pool, "smtp:run-1:a@b.com")
            .await
            .expect("get")
            .expect("row");
        assert_eq!(stored.status, "SUCCESS");
        assert_eq!(stored.attempts, 1);
        assert_eq!(stored.external_id.as_deref(), Some("msg-1"));
    }
}
