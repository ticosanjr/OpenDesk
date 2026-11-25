//! SQLite integrity preflight and safe repair.
//!
//! A corrupt database must never reach migrations or the UI. This module probes
//! `quick_check`, preserves every original file, and only swaps in a repaired
//! copy after `integrity_check` returns `ok`.

use crate::error::{AppError, AppResult};
use chrono::Utc;
use serde::Serialize;
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct DbIntegrityReport {
    pub ok: bool,
    pub journal_mode: String,
    pub errors: Vec<String>,
}

async fn open(path: &Path) -> AppResult<SqliteConnection> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(5));
    SqliteConnection::connect_with(&options)
        .await
        .map_err(|error| {
            AppError::Internal(format!(
                "não foi possível abrir o banco {}: {error}",
                path.display()
            ))
        })
}

async fn quick_check(path: &Path) -> AppResult<DbIntegrityReport> {
    if !path.exists() {
        return Ok(DbIntegrityReport {
            ok: true,
            journal_mode: "absent".into(),
            errors: Vec::new(),
        });
    }
    let mut connection = open(path).await?;
    let journal_mode = sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
        .fetch_one(&mut connection)
        .await
        .unwrap_or_else(|_| "unknown".into());
    let rows = sqlx::query_scalar::<_, String>("PRAGMA quick_check(5)")
        .fetch_all(&mut connection)
        .await;
    match rows {
        Ok(rows) if rows.len() == 1 && rows[0].eq_ignore_ascii_case("ok") => {
            Ok(DbIntegrityReport {
                ok: true,
                journal_mode,
                errors: Vec::new(),
            })
        }
        Ok(rows) => Ok(DbIntegrityReport {
            ok: false,
            journal_mode,
            errors: rows,
        }),
        Err(error) => Ok(DbIntegrityReport {
            ok: false,
            journal_mode,
            errors: vec![error.to_string()],
        }),
    }
}

pub async fn check_database_integrity(db_path: &Path) -> AppResult<DbIntegrityReport> {
    quick_check(db_path).await
}

fn backup_directory(db_path: &Path) -> PathBuf {
    let name = db_path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "opendesk.db".into());
    db_path.with_file_name(format!(
        "{name}.corrupt-{}",
        Utc::now().format("%Y%m%d-%H%M%S")
    ))
}

fn backup_files(db_path: &Path) -> AppResult<PathBuf> {
    let directory = backup_directory(db_path);
    std::fs::create_dir_all(&directory)?;
    for suffix in ["", "-wal", "-shm"] {
        let source = PathBuf::from(format!("{}{suffix}", db_path.display()));
        if !source.exists() {
            continue;
        }
        let name = source
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "opendesk.db".into());
        std::fs::copy(&source, directory.join(name))?;
    }
    Ok(directory)
}

fn remove_sidecars(db_path: &Path) {
    for suffix in ["-wal", "-shm"] {
        let path = PathBuf::from(format!("{}{suffix}", db_path.display()));
        let _ = std::fs::remove_file(path);
    }
}

async fn vacuum_into(source: &Path, target: &Path) -> AppResult<()> {
    if target.exists() {
        let _ = std::fs::remove_file(target);
    }
    let mut connection = open(source).await?;
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut connection)
        .await
        .ok();
    let statement = format!(
        "VACUUM INTO '{}'",
        target.display().to_string().replace('\'', "''")
    );
    sqlx::query(&statement)
        .execute(&mut connection)
        .await
        .map_err(|error| AppError::Internal(format!("VACUUM INTO falhou: {error}")))?;
    Ok(())
}

fn recover_with_cli(source: &Path, target: &Path) -> AppResult<()> {
    if target.exists() {
        let _ = std::fs::remove_file(target);
    }
    let dump = std::process::Command::new("sqlite3")
        .arg(source)
        .arg(".recover")
        .output()
        .map_err(|_| {
            AppError::Internal(
                "VACUUM INTO falhou e o comando `sqlite3` não está disponível; \
                 instale sqlite3 ou restaure o backup automático"
                    .into(),
            )
        })?;
    if !dump.status.success() {
        return Err(AppError::Internal(format!(
            "sqlite3 .recover falhou: {}",
            String::from_utf8_lossy(&dump.stderr)
        )));
    }
    let mut child = std::process::Command::new("sqlite3")
        .arg(target)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| AppError::Internal(format!("sqlite3 reparo: {error}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin
            .write_all(&dump.stdout)
            .map_err(|error| AppError::Internal(format!("sqlite3 dump: {error}")))?;
    }
    let status = child
        .wait()
        .map_err(|error| AppError::Internal(format!("sqlite3 reparo: {error}")))?;
    if !status.success() {
        return Err(AppError::Internal(
            "sqlite3 não conseguiu aplicar o dump".into(),
        ));
    }
    Ok(())
}

async fn validate(path: &Path) -> AppResult<()> {
    let mut connection = open(path).await?;
    let row = sqlx::query_scalar::<_, String>("PRAGMA integrity_check")
        .fetch_one(&mut connection)
        .await
        .map_err(|error| AppError::Internal(format!("integrity_check: {error}")))?;
    if row.eq_ignore_ascii_case("ok") {
        Ok(())
    } else {
        Err(AppError::Internal(format!(
            "banco reparado ainda está inválido: {row}"
        )))
    }
}

pub async fn repair_database(db_path: &Path) -> AppResult<DbIntegrityReport> {
    if !db_path.exists() {
        return check_database_integrity(db_path).await;
    }
    let backup = backup_files(db_path)?;
    let repaired = db_path.with_file_name(format!(
        "{}.repaired-{}",
        db_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "opendesk.db".into()),
        Utc::now().format("%Y%m%d-%H%M%S")
    ));
    let result = async {
        if vacuum_into(db_path, &repaired).await.is_err() {
            recover_with_cli(db_path, &repaired)?;
        }
        validate(&repaired).await?;
        remove_sidecars(db_path);
        std::fs::rename(&repaired, db_path)?;
        Ok::<(), AppError>(())
    }
    .await;
    if let Err(error) = result {
        let _ = std::fs::remove_file(&repaired);
        return Err(AppError::Internal(format!(
            "não foi possível reparar o banco (backup em {}): {error}",
            backup.display()
        )));
    }
    check_database_integrity(db_path).await
}

/// Boot preflight: repair a corrupt database before migrations open it.
pub async fn ensure_database_integrity(db_path: &Path) -> AppResult<DbIntegrityReport> {
    let report = check_database_integrity(db_path).await?;
    if report.ok {
        return Ok(report);
    }
    tracing::error!(
        errors = ?report.errors,
        db = %db_path.display(),
        "SQLite corrompido detectado; criando backup e tentando reparo"
    );
    repair_database(db_path).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn missing_and_healthy_databases_pass_the_preflight() {
        let temp = TempDir::new().expect("temp");
        let missing = temp.path().join("missing.db");
        let report = check_database_integrity(&missing).await.expect("missing");
        assert!(report.ok);

        let path = temp.path().join("healthy.db");
        let pool = crate::init(&path).await.expect("init");
        pool.close().await;
        let report = check_database_integrity(&path).await.expect("healthy");
        assert!(report.ok, "{:?}", report.errors);
    }

    #[tokio::test]
    async fn corrupt_database_fails_closed_and_keeps_a_backup() {
        let temp = TempDir::new().expect("temp");
        let path = temp.path().join("corrupt.db");
        std::fs::write(&path, b"not a sqlite database at all").expect("corrupt");
        let report = check_database_integrity(&path).await.expect("check");
        assert!(!report.ok);
        let error = ensure_database_integrity(&path).await.expect_err("repair");
        assert!(error.to_string().contains("reparar") || error.to_string().contains("backup"));
        let backups = std::fs::read_dir(temp.path())
            .expect("dir")
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().contains(".corrupt-"))
            .count();
        assert_eq!(backups, 1);
        assert!(path.exists(), "original is never removed on failure");
    }
}
