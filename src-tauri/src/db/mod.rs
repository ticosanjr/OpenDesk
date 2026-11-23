//! Shell database facade over storage repositories and maintenance.

use sqlx::SqlitePool;
use std::path::Path;

pub use opendesk_engine::worktree::store as worktrees;
pub use opendesk_storage::repos::*;
pub use opendesk_storage::write_buffer;

pub use opendesk_storage::maintenance;

/// Opens the V2 database. The schema lives in `opendesk-storage`.
pub async fn init(db_path: &Path) -> anyhow::Result<SqlitePool> {
    opendesk_storage::init(db_path).await
}

#[allow(dead_code)]
pub async fn ping(pool: &SqlitePool) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(pool)
        .await
        .map(|value| value == 1)
        .unwrap_or(false)
}
