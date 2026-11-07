use crate::{error::AppResult, paths::AppPaths};
use sqlx::SqlitePool;
use std::time::{Duration, SystemTime};

const BACKUP_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const LOG_RETENTION_DAYS: u64 = 30;

pub async fn boot_check(pool: &SqlitePool) -> AppResult<()> {
    let result: String = sqlx::query_scalar("PRAGMA quick_check")
        .fetch_one(pool)
        .await?;
    if result != "ok" {
        tracing::error!(result, "SQLite quick_check failed");
    }
    Ok(())
}

pub async fn run_loop(pool: SqlitePool, paths: AppPaths) {
    // First pass shortly after boot, then every six hours.
    tokio::time::sleep(Duration::from_secs(300)).await;
    loop {
        if let Err(error) = maintenance_once(&pool, &paths).await {
            tracing::warn!(error = %error, "SQLite maintenance failed");
        }
        tokio::time::sleep(BACKUP_INTERVAL).await;
    }
}

pub async fn maintenance_once(pool: &SqlitePool, paths: &AppPaths) -> AppResult<()> {
    let _ = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(pool)
        .await;
    let _ = sqlx::query("PRAGMA optimize").execute(pool).await;

    std::fs::create_dir_all(&paths.backups_dir)?;
    let backup = paths.backups_dir.join(format!(
        "opendesk-{}.db",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    ));
    if !backup.exists() {
        let statement = format!(
            "VACUUM INTO '{}'",
            backup.to_string_lossy().replace('\'', "''")
        );
        if let Err(error) = sqlx::query(&statement).execute(pool).await {
            tracing::warn!(error = %error, "VACUUM INTO failed");
        }
    }
    prune_backups(&paths.backups_dir, 3, 7)?;
    prune_run_logs(&paths.run_logs_dir, LOG_RETENTION_DAYS)?;
    Ok(())
}

fn prune_backups(dir: &std::path::Path, keep: usize, days: u64) -> AppResult<()> {
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(days * 24 * 60 * 60))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut files = std::fs::read_dir(dir)?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            if !name.starts_with("opendesk-") || !name.ends_with(".db") {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((path, modified))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(_, modified)| *modified);
    let remove_count = files.len().saturating_sub(keep);
    for (path, _) in files.iter().take(remove_count) {
        let _ = std::fs::remove_file(path);
    }
    for (path, modified) in files.into_iter().skip(remove_count) {
        if modified < cutoff {
            let _ = std::fs::remove_file(path);
        }
    }
    Ok(())
}

fn prune_run_logs(dir: &std::path::Path, days: u64) -> AppResult<()> {
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(days * 24 * 60 * 60))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Ok(metadata) = entry.metadata()
                && metadata.modified().map(|at| at < cutoff).unwrap_or(false)
            {
                let _ = std::fs::remove_file(path);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn prunes_old_logs_and_backups() {
        let temp = TempDir::new().expect("temp");
        let logs = temp.path().join("runs");
        let backups = temp.path().join("backups");
        std::fs::create_dir_all(&logs).expect("logs");
        std::fs::create_dir_all(&backups).expect("backups");
        let old = logs.join("old.jsonl");
        std::fs::write(&old, "x").expect("old");
        prune_run_logs(&logs, 0).expect("prune");
        assert!(!old.exists());
        for index in 0..5 {
            std::fs::write(backups.join(format!("opendesk-{index}.db")), "x").expect("backup");
        }
        prune_backups(&backups, 3, 7).expect("prune backups");
        assert_eq!(std::fs::read_dir(&backups).expect("read").count(), 3);
    }
}
