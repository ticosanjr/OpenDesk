//! Queries used by the one-time I/O-bus secret migration.

use super::{INBOUND_COLUMNS, InboundSource, OUTBOUND_COLUMNS, OutboundSink};
use crate::error::AppResult;
use sqlx::SqlitePool;

/// Every inbound row, including disabled ones. Used by the secret migration so
/// legacy plaintext never survives just because a source is turned off.
pub async fn all_inbound(pool: &SqlitePool) -> AppResult<Vec<InboundSource>> {
    let query =
        format!("SELECT {INBOUND_COLUMNS} FROM io_inbound_sources ORDER BY workspace_id, name");
    Ok(sqlx::query_as::<_, InboundSource>(&query)
        .fetch_all(pool)
        .await?)
}

pub async fn all_outbound(pool: &SqlitePool) -> AppResult<Vec<OutboundSink>> {
    let query =
        format!("SELECT {OUTBOUND_COLUMNS} FROM io_outbound_sinks ORDER BY workspace_id, name");
    Ok(sqlx::query_as::<_, OutboundSink>(&query)
        .fetch_all(pool)
        .await?)
}

pub async fn set_inbound_config(pool: &SqlitePool, id: &str, config_json: &str) -> AppResult<()> {
    sqlx::query("UPDATE io_inbound_sources SET config_json = ? WHERE id = ?")
        .bind(config_json)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_outbound_config(pool: &SqlitePool, id: &str, config_json: &str) -> AppResult<()> {
    sqlx::query("UPDATE io_outbound_sinks SET config_json = ? WHERE id = ?")
        .bind(config_json)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
