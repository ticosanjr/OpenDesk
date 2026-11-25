use anyhow::{Context, Result, bail};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use std::{path::Path, time::Duration};

pub const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_READ_CONNECTIONS: u32 = 5;
const CACHE_SIZE_KIB: &str = "-64000";

#[derive(Debug, Clone)]
pub struct StorageOptions {
    pub read_connections: u32,
    pub busy_timeout: Duration,
}

impl Default for StorageOptions {
    fn default() -> Self {
        Self {
            read_connections: DEFAULT_READ_CONNECTIONS,
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
        }
    }
}

/// Read pool + serialized writer over the same database file.
#[derive(Debug)]
pub struct Storage {
    pub read: SqlitePool,
    pub write: SqlitePool,
}

impl Storage {
    pub async fn open(path: &Path) -> Result<Self> {
        Self::open_with(path, StorageOptions::default()).await
    }

    pub async fn open_with(path: &Path, options: StorageOptions) -> Result<Self> {
        crate::integrity::ensure_database_integrity(path)
            .await
            .map_err(|error| anyhow::anyhow!(error))?;
        let existed = path.exists();
        let write = open_write_pool(path, options.busy_timeout).await?;
        prepare(&write, path, existed).await?;
        let read = open_read_pool(path, options.read_connections, options.busy_timeout).await?;
        Ok(Self { read, write })
    }

    pub async fn close(self) {
        self.read.close().await;
        self.write.close().await;
    }
}

/// Single-pool entry point kept for the V1 shell (`opendesk_core::db::init`).
pub async fn init(db_path: &Path) -> Result<SqlitePool> {
    crate::integrity::ensure_database_integrity(db_path)
        .await
        .map_err(|error| anyhow::anyhow!(error))?;
    let existed = db_path.exists();
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("could not create database directory {}", parent.display()))?;
    }
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(write_options(db_path, DEFAULT_BUSY_TIMEOUT))
        .await
        .with_context(|| format!("could not open SQLite database {}", db_path.display()))?;
    prepare(&pool, db_path, existed).await?;
    Ok(pool)
}

fn write_options(path: &Path, busy_timeout: Duration) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        // WAL + NORMAL keeps the database consistent and drops an fsync per
        // commit. A power loss may lose the last transactions, not corrupt.
        .synchronous(SqliteSynchronous::Normal)
        .foreign_keys(true)
        .busy_timeout(busy_timeout)
        .pragma("cache_size", CACHE_SIZE_KIB)
}

fn read_options(path: &Path, busy_timeout: Duration) -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .foreign_keys(true)
        .busy_timeout(busy_timeout)
        .pragma("cache_size", CACHE_SIZE_KIB)
        // The read pool must never write; this turns an accidental write into
        // an immediate error instead of a lock.
        .pragma("query_only", "true")
}

async fn open_write_pool(path: &Path, busy_timeout: Duration) -> Result<SqlitePool> {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(write_options(path, busy_timeout))
        .await
        .with_context(|| format!("could not open SQLite writer {}", path.display()))
}

async fn open_read_pool(
    path: &Path,
    connections: u32,
    busy_timeout: Duration,
) -> Result<SqlitePool> {
    SqlitePoolOptions::new()
        .max_connections(connections.max(1))
        .connect_with(read_options(path, busy_timeout))
        .await
        .with_context(|| format!("could not open SQLite reader {}", path.display()))
}

async fn prepare(pool: &SqlitePool, path: &Path, existed: bool) -> Result<()> {
    guard_legacy(pool, path).await?;
    let non_empty = existed
        && std::fs::metadata(path)
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false);
    if non_empty {
        backup_existing_database(pool, path).await?;
    }
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("database migration failed")?;
    Ok(())
}

/// V1 databases have `_sqlx_migrations` with versions 1..59 and no `v2_meta`.
/// Applying the V2 baseline over one would be destructive, so refuse with an
/// actionable message instead.
async fn guard_legacy(pool: &SqlitePool, path: &Path) -> Result<()> {
    let has_migrations: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await
    .unwrap_or(0);
    if has_migrations == 0 {
        return Ok(());
    }
    let v2 =
        sqlx::query_scalar::<_, String>("SELECT value FROM v2_meta WHERE key = 'schema_version'")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    if v2.is_some() {
        return Ok(());
    }
    let max_version: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(pool)
            .await
            .unwrap_or(0);
    if max_version > 0 {
        bail!(
            "banco OpenDesk V1 detectado (schema {max_version}) em {}. Exporte com \
             `opendesk migrate-v1-export --db {} --out <dir>` e inicie o V2 com um arquivo novo.",
            path.display(),
            path.display()
        );
    }
    Ok(())
}

/// Keep the last three copies of a V2 database next to the original file.
async fn backup_existing_database(pool: &SqlitePool, db_path: &Path) -> Result<()> {
    // Fold the WAL into the main file; a plain copy can miss committed pages.
    if let Err(error) = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_optional(pool)
        .await
    {
        tracing::warn!(error = %error, "could not checkpoint the WAL before backup");
    }
    let backup = db_path.with_extension("db.bak");
    let second = db_path.with_extension("db.bak.2");
    let third = db_path.with_extension("db.bak.3");
    if backup.exists() {
        let _ = std::fs::copy(&backup, &third);
        let _ = std::fs::copy(&backup, &second);
    }
    std::fs::copy(db_path, &backup)
        .with_context(|| format!("could not back up database to {}", backup.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn baseline_is_strict_and_integrity_clean() {
        let dir = tempfile::TempDir::new().expect("temp");
        let pool = init(&dir.path().join("v2.db")).await.expect("init");

        let non_strict: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
               AND name NOT IN ('_sqlx_migrations')
               AND sql NOT LIKE '%STRICT%'",
        )
        .fetch_one(&pool)
        .await
        .expect("tables");
        assert_eq!(non_strict, 0, "every table must be STRICT");

        let foreign_keys = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .expect("fk");
        assert!(
            foreign_keys.is_empty(),
            "{} violações de FK",
            foreign_keys.len()
        );

        let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&pool)
            .await
            .expect("integrity");
        assert_eq!(integrity, "ok");

        let version: String =
            sqlx::query_scalar("SELECT value FROM v2_meta WHERE key = 'schema_version'")
                .fetch_one(&pool)
                .await
                .expect("version");
        assert_eq!(version, "2");
    }

    #[tokio::test]
    async fn connection_pragmas_match_the_contract() {
        let dir = tempfile::TempDir::new().expect("temp");
        let storage = Storage::open(&dir.path().join("v2.db"))
            .await
            .expect("storage");

        let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&storage.write)
            .await
            .expect("journal");
        assert_eq!(journal.to_lowercase(), "wal");

        let synchronous: i64 = sqlx::query_scalar("PRAGMA synchronous")
            .fetch_one(&storage.write)
            .await
            .expect("synchronous");
        assert_eq!(synchronous, 1, "NORMAL");

        let busy: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
            .fetch_one(&storage.write)
            .await
            .expect("busy");
        assert_eq!(busy, 5000);

        let cache: i64 = sqlx::query_scalar("PRAGMA cache_size")
            .fetch_one(&storage.write)
            .await
            .expect("cache");
        assert_eq!(cache, -64000);

        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&storage.write)
            .await
            .expect("fk");
        assert_eq!(foreign_keys, 1);

        let read_only: i64 = sqlx::query_scalar("PRAGMA query_only")
            .fetch_one(&storage.read)
            .await
            .expect("query_only");
        assert_eq!(read_only, 1);

        assert!(
            sqlx::query("CREATE TABLE nope (id TEXT) STRICT")
                .execute(&storage.read)
                .await
                .is_err(),
            "the read pool must not write"
        );
        storage.close().await;
    }

    #[tokio::test]
    async fn refuses_a_v1_database() {
        let dir = tempfile::TempDir::new().expect("temp");
        let path = dir.path().join("v1.db");
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true),
            )
            .await
            .expect("v1");
        sqlx::query(
            "CREATE TABLE _sqlx_migrations (
                version INTEGER PRIMARY KEY, description TEXT NOT NULL,
                installed_on TEXT NOT NULL DEFAULT '', success INTEGER NOT NULL,
                checksum BLOB NOT NULL, execution_time INTEGER NOT NULL
             )",
        )
        .execute(&pool)
        .await
        .expect("migrations");
        sqlx::query(
            "INSERT INTO _sqlx_migrations
                (version, description, success, checksum, execution_time)
             VALUES (59, 'skill shadow', 1, x'00', 0)",
        )
        .execute(&pool)
        .await
        .expect("version");
        pool.close().await;

        let error = Storage::open(&path).await.expect_err("v1 refused");
        let message = format!("{error:#}");
        assert!(message.contains("V1"), "{message}");
        assert!(message.contains("migrate-v1-export"), "{message}");
    }

    #[tokio::test]
    async fn backup_includes_pages_that_still_live_in_the_wal() {
        let dir = tempfile::TempDir::new().expect("temp");
        let path = dir.path().join("v2.db");
        let pool = init(&path).await.expect("init");
        sqlx::query(
            "INSERT INTO workspaces (id, name, root_path) VALUES ('wal-ws', 'WAL', '/tmp/wal')",
        )
        .execute(&pool)
        .await
        .expect("workspace");
        let second = init(&path).await.expect("second init");

        let backup_path = path.with_extension("db.bak");
        assert!(backup_path.exists());
        let backup = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&backup_path)
                    .read_only(true),
            )
            .await
            .expect("backup");
        let name: String = sqlx::query_scalar("SELECT name FROM workspaces WHERE id = 'wal-ws'")
            .fetch_one(&backup)
            .await
            .expect("row");
        assert_eq!(name, "WAL");
        backup.close().await;
        second.close().await;
        pool.close().await;
    }
}
