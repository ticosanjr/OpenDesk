use anyhow::{Context, Result};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use std::{path::Path, str::FromStr, time::Duration};

pub mod artifacts;
pub mod budget;
pub mod chat;
pub mod deliveries;
pub mod integrations;
pub mod model_health;
pub mod models;
pub mod permissions;
pub mod queue;
pub mod router;
pub mod runs;
pub mod schedules;
pub mod settings;
pub mod task_messages;
pub mod tasks;
pub mod workspaces;

pub async fn init(db_path: &Path) -> Result<SqlitePool> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not create database directory {}", parent.display()))?;
    }

    backup_existing_database(db_path)?;

    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", db_path.display()))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .with_context(|| format!("could not open SQLite database {}", db_path.display()))?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("database migration failed")?;

    Ok(pool)
}

/// Keep the last three copies of the database next to the original file. This
/// runs before migrations, so a failed upgrade always has a rollback point.
fn backup_existing_database(db_path: &Path) -> Result<()> {
    if !db_path.exists() {
        return Ok(());
    }
    let metadata = std::fs::metadata(db_path)
        .with_context(|| format!("could not stat database {}", db_path.display()))?;
    if metadata.len() == 0 {
        return Ok(());
    }
    let backup = db_path.with_extension("db.bak");
    let second = db_path.with_extension("db.bak.2");
    let third = db_path.with_extension("db.bak.3");
    if backup.exists() {
        let _ = std::fs::copy(&backup, &third);
    }
    if backup.exists() {
        let _ = std::fs::copy(&backup, &second);
    }
    std::fs::copy(db_path, &backup)
        .with_context(|| format!("could not back up database to {}", backup.display()))?;
    Ok(())
}

#[allow(dead_code)]
pub async fn ping(pool: &SqlitePool) -> bool {
    sqlx::query_scalar::<_, i64>("SELECT 1")
        .fetch_one(pool)
        .await
        .map(|value| value == 1)
        .unwrap_or(false)
}

#[cfg(test)]
mod migration_tests {
    #[tokio::test]
    async fn all_migrations_apply() {
        let dir = tempfile::TempDir::new().unwrap();
        let pool = super::init(&dir.path().join("m.db")).await.unwrap();
        let rows: Vec<(i64, String)> =
            sqlx::query_as("SELECT version, description FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(rows.last().unwrap().0, 15, "rows: {rows:?}");
        let cols: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM pragma_table_info('execution_queue')")
                .fetch_all(&pool)
                .await
                .unwrap();
        let names: Vec<&str> = cols.iter().map(|c| c.0.as_str()).collect();
        assert!(names.contains(&"concurrency_group"), "cols: {names:?}");
    }
}
