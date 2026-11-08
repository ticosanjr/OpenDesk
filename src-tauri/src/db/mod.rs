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
pub mod io_bus;
pub mod maintenance;
pub mod model_health;
pub mod models;
pub mod permissions;
pub mod proposals;
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
        // Background workers (watcher, queue, maintenance) share this pool with
        // the API. A 5s budget was short enough to surface `database is locked`
        // under bursts of writes, so leave room for the writer to drain.
        .busy_timeout(Duration::from_secs(15));

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
    use super::*;

    #[tokio::test]
    async fn upgrades_a_legacy_database_to_the_latest_schema() {
        use sqlx::migrate::Migrator;
        let dir = tempfile::TempDir::new().unwrap();
        let partial = dir.path().join("partial-migrations");
        std::fs::create_dir_all(&partial).unwrap();
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
        for entry in std::fs::read_dir(&source).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(version) = name
                .split('_')
                .next()
                .and_then(|value| value.parse::<i64>().ok())
                && version <= 19
            {
                std::fs::copy(entry.path(), partial.join(name)).unwrap();
            }
        }
        let db_path = dir.path().join("legacy.db");
        let options =
            sqlx::sqlite::SqliteConnectOptions::from_str(&format!("sqlite:{}", db_path.display()))
                .unwrap()
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal)
                .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        Migrator::new(partial)
            .await
            .unwrap()
            .run(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('legacy', 'Legacy', '/tmp/legacy')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO tasks (id, workspace_id, title, description, status) VALUES ('legacy-task', 'legacy', 'Velha', 'x', 'BACKLOG')")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;

        let migrated = super::init(&db_path).await.unwrap();
        let status: String =
            sqlx::query_scalar("SELECT status FROM tasks WHERE id = 'legacy-task'")
                .fetch_one(&migrated)
                .await
                .unwrap();
        assert_eq!(status, "TODO");
        let schedule_cols: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM pragma_table_info('schedules')")
                .fetch_all(&migrated)
                .await
                .unwrap();
        let names: Vec<&str> = schedule_cols.iter().map(|c| c.0.as_str()).collect();
        for column in [
            "trigger_type",
            "trigger_config_json",
            "target_kind",
            "recipe_id",
        ] {
            assert!(names.contains(&column), "missing {column}: {names:?}");
        }
        let alert: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'alert_dismissals'",
        )
        .fetch_one(&migrated)
        .await
        .unwrap();
        assert_eq!(alert, 1);
    }

    #[tokio::test]
    async fn all_migrations_apply() {
        let dir = tempfile::TempDir::new().unwrap();
        let pool = super::init(&dir.path().join("m.db")).await.unwrap();
        let rows: Vec<(i64, String)> =
            sqlx::query_as("SELECT version, description FROM _sqlx_migrations ORDER BY version")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(rows.last().unwrap().0, 22, "rows: {rows:?}");
        let cols: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM pragma_table_info('execution_queue')")
                .fetch_all(&pool)
                .await
                .unwrap();
        let names: Vec<&str> = cols.iter().map(|c| c.0.as_str()).collect();
        assert!(names.contains(&"concurrency_group"), "cols: {names:?}");
    }
}
