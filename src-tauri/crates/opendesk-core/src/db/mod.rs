use sqlx::SqlitePool;
use std::path::Path;

pub use opendesk_storage::repos::ab;
pub use opendesk_storage::repos::artifacts;
pub use opendesk_storage::repos::auth_sessions;
pub use opendesk_storage::repos::budget;
pub use opendesk_storage::repos::calendar;
pub use opendesk_storage::repos::channels;
pub mod chat;
pub mod chat_turns;
pub use opendesk_storage::repos::deliveries;
pub use opendesk_storage::repos::handoffs;
pub use opendesk_storage::repos::integrations;
pub mod interactions;
pub use opendesk_storage::repos::io_bus;
pub mod maintenance;
pub use opendesk_storage::repos::memory;
pub use opendesk_storage::repos::missed_runs;
pub use opendesk_storage::repos::model_health;
pub use opendesk_storage::repos::models;
pub use opendesk_storage::repos::permissions;
pub use opendesk_storage::repos::proposals;
pub use opendesk_storage::repos::push;
pub use opendesk_storage::repos::queue;
pub use opendesk_storage::repos::recipe_files;
pub use opendesk_storage::repos::router;
pub use opendesk_storage::repos::runs;
pub use opendesk_storage::repos::schedules;
pub use opendesk_storage::repos::settings;
pub use opendesk_storage::repos::task_dedupe;
pub mod task_messages;
pub use opendesk_storage::repos::task_watchers;
pub use opendesk_storage::repos::tasks;
pub use opendesk_storage::repos::tickets;
pub use opendesk_storage::repos::workspaces;
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
