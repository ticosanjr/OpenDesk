use crate::error::{AppError, AppResult};
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct PushSubscription {
    pub id: String,
    pub endpoint: String,
    #[serde(skip_serializing)]
    pub p256dh: String,
    #[serde(skip_serializing)]
    pub auth: String,
    pub label: String,
    pub created_at: String,
    pub last_success_at: Option<String>,
    pub last_failure_at: Option<String>,
    pub failure_count: i64,
}

const COLUMNS: &str = "id, endpoint, p256dh, auth, label, created_at, last_success_at, last_failure_at, failure_count";

/// Stores or refreshes a browser subscription. Browsers may rotate the keys for
/// an existing endpoint, so the unique endpoint wins over a fresh id.
pub async fn upsert(
    pool: &SqlitePool,
    endpoint: &str,
    p256dh: &str,
    auth: &str,
    label: &str,
) -> AppResult<PushSubscription> {
    let endpoint = endpoint.trim();
    if !endpoint.starts_with("https://") {
        return Err(AppError::BadRequest(
            "endpoint de push precisa usar https".into(),
        ));
    }
    if p256dh.trim().is_empty() || auth.trim().is_empty() {
        return Err(AppError::BadRequest(
            "chaves de criptografia do push ausentes".into(),
        ));
    }
    let label = label.trim();
    let label = if label.is_empty() { "device" } else { label };

    let existing: Option<String> =
        sqlx::query_scalar("SELECT id FROM push_subscriptions WHERE endpoint = ?")
            .bind(endpoint)
            .fetch_optional(pool)
            .await?;
    let id = existing.unwrap_or_else(|| Uuid::now_v7().to_string());
    sqlx::query(
        "INSERT INTO push_subscriptions (id, endpoint, p256dh, auth, label)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(endpoint) DO UPDATE SET
             p256dh = excluded.p256dh,
             auth = excluded.auth,
             label = excluded.label,
             failure_count = 0,
             last_failure_at = NULL",
    )
    .bind(&id)
    .bind(endpoint)
    .bind(p256dh.trim())
    .bind(auth.trim())
    .bind(label)
    .execute(pool)
    .await?;
    get(pool, &id).await
}

pub async fn list(pool: &SqlitePool) -> AppResult<Vec<PushSubscription>> {
    let query = format!("SELECT {COLUMNS} FROM push_subscriptions ORDER BY created_at DESC");
    Ok(sqlx::query_as::<_, PushSubscription>(&query)
        .fetch_all(pool)
        .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<PushSubscription> {
    let query = format!("SELECT {COLUMNS} FROM push_subscriptions WHERE id = ?");
    sqlx::query_as::<_, PushSubscription>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("push subscription {id}")))
}

pub async fn remove(pool: &SqlitePool, endpoint: &str) -> AppResult<bool> {
    let result = sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = ?")
        .bind(endpoint)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn remove_by_id(pool: &SqlitePool, id: &str) -> AppResult<bool> {
    let result = sqlx::query("DELETE FROM push_subscriptions WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn mark_success(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE push_subscriptions
         SET last_success_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'), failure_count = 0
         WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Counts consecutive failures. Endpoints the push service has permanently
/// rejected (404/410) should be removed by the caller instead.
pub async fn mark_failure(pool: &SqlitePool, id: &str) -> AppResult<i64> {
    sqlx::query(
        "UPDATE push_subscriptions
         SET last_failure_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
             failure_count = failure_count + 1
         WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    let count: i64 =
        sqlx::query_scalar("SELECT failure_count FROM push_subscriptions WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?
            .unwrap_or(0);
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn upserts_lists_and_removes_subscriptions() {
        let ctx = crate::test_support::context().await;
        let first = upsert(
            &ctx.state.db,
            "https://push.example/abc",
            "p256dh",
            "auth",
            "phone",
        )
        .await
        .expect("subscription");
        assert_eq!(first.label, "phone");
        let second = upsert(
            &ctx.state.db,
            "https://push.example/abc",
            "p256dh2",
            "auth2",
            "tablet",
        )
        .await
        .expect("refresh");
        assert_eq!(second.id, first.id);
        assert_eq!(second.label, "tablet");
        assert_eq!(list(&ctx.state.db).await.expect("list").len(), 1);
        assert!(
            remove(&ctx.state.db, "https://push.example/abc")
                .await
                .expect("remove")
        );
        assert!(list(&ctx.state.db).await.expect("list").is_empty());
    }

    #[tokio::test]
    async fn rejects_non_https_endpoints() {
        let ctx = crate::test_support::context().await;
        let error = upsert(&ctx.state.db, "http://push.example", "a", "b", "x")
            .await
            .expect_err("must reject plain http");
        assert!(matches!(error, AppError::BadRequest(_)));
    }
}
