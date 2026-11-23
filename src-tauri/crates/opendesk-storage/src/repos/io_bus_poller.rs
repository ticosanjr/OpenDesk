//! HTTP/RSS poller state and deduplication.

use crate::error::AppResult;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PollerState {
    pub source_id: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub last_polled_at: Option<String>,
    pub last_error: Option<String>,
}

pub async fn poller_state(pool: &SqlitePool, source_id: &str) -> AppResult<Option<PollerState>> {
    Ok(sqlx::query_as::<_, PollerState>(
        "SELECT source_id, etag, last_modified, last_polled_at, last_error
         FROM io_poller_state WHERE source_id = ?",
    )
    .bind(source_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn save_poller_state(
    pool: &SqlitePool,
    source_id: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
    error: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO io_poller_state (source_id, etag, last_modified, last_polled_at, last_error)
         VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?)
         ON CONFLICT(source_id) DO UPDATE SET
            etag = excluded.etag,
            last_modified = excluded.last_modified,
            last_polled_at = excluded.last_polled_at,
            last_error = excluded.last_error",
    )
    .bind(source_id)
    .bind(etag)
    .bind(last_modified)
    .bind(error)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn poller_seen_hash(
    pool: &SqlitePool,
    source_id: &str,
    item_key: &str,
) -> AppResult<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT content_hash FROM io_poller_seen WHERE source_id = ? AND item_key = ?",
    )
    .bind(source_id)
    .bind(item_key)
    .fetch_optional(pool)
    .await?)
}

pub async fn mark_poller_seen(
    pool: &SqlitePool,
    source_id: &str,
    item_key: &str,
    content_hash: &str,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO io_poller_seen (source_id, item_key, content_hash)
         VALUES (?, ?, ?)
         ON CONFLICT(source_id, item_key) DO UPDATE SET
            content_hash = excluded.content_hash,
            seen_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(source_id)
    .bind(item_key)
    .bind(content_hash)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn prune_poller_seen(pool: &SqlitePool, source_id: &str, keep: i64) -> AppResult<()> {
    sqlx::query(
        "DELETE FROM io_poller_seen WHERE source_id = ? AND item_key NOT IN (
            SELECT item_key FROM io_poller_seen WHERE source_id = ?
            ORDER BY seen_at DESC LIMIT ?
         )",
    )
    .bind(source_id)
    .bind(source_id)
    .bind(keep.clamp(1, 5000))
    .execute(pool)
    .await?;
    Ok(())
}
