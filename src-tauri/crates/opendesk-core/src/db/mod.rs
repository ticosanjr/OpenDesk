use sqlx::SqlitePool;
use std::path::Path;

pub mod ab;
pub mod artifacts;
pub mod auth_sessions;
pub mod budget;
pub mod calendar;
pub mod channels;
pub mod chat;
pub mod chat_turns;
pub mod deliveries;
pub mod handoffs;
pub mod integrations;
pub mod interactions;
pub mod io_bus;
pub mod maintenance;
pub mod memory;
pub mod missed_runs;
pub mod model_health;
pub mod models;
pub mod permissions;
pub mod proposals;
pub mod push;
pub mod queue;
pub mod recipe_files;
pub mod router;
pub mod runs;
pub mod schedules;
pub mod settings;
pub mod task_dedupe;
pub mod task_messages;
pub mod task_watchers;
pub mod tasks;
pub mod tickets;
pub mod workspaces;
pub mod worktrees;
pub mod write_buffer;

/// Opens the V2 database. The schema lives in `opendesk-storage`; the 59 V1
/// migrations were replaced by one STRICT baseline.
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
