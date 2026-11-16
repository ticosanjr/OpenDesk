use crate::{
    error::{AppError, AppResult},
    worktree::RunWorktree,
};
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, FromRow)]
pub struct WorktreeRow {
    pub id: String,
    pub workspace_id: String,
    pub run_id: Option<String>,
    pub path: String,
    pub mode: String,
    pub branch: Option<String>,
    pub base_commit: Option<String>,
    pub status: String,
    pub created_at: String,
    pub merged_at: Option<String>,
}

impl WorktreeRow {
    pub fn into_model(self) -> RunWorktree {
        RunWorktree {
            id: self.id,
            workspace_id: self.workspace_id,
            run_id: self.run_id,
            path: self.path,
            mode: self.mode,
            branch: self.branch,
            base_commit: self.base_commit,
            status: self.status,
            created_at: self.created_at,
            merged_at: self.merged_at,
        }
    }
}

pub async fn insert(pool: &SqlitePool, worktree: &RunWorktree) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO run_worktrees
            (id, workspace_id, run_id, path, mode, branch, base_commit, status, created_at, merged_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&worktree.id)
    .bind(&worktree.workspace_id)
    .bind(&worktree.run_id)
    .bind(&worktree.path)
    .bind(&worktree.mode)
    .bind(&worktree.branch)
    .bind(&worktree.base_commit)
    .bind(&worktree.status)
    .bind(&worktree.created_at)
    .bind(&worktree.merged_at)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<RunWorktree> {
    sqlx::query_as::<_, WorktreeRow>("SELECT * FROM run_worktrees WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?
        .map(WorktreeRow::into_model)
        .ok_or_else(|| AppError::NotFound(format!("worktree {id}")))
}

pub async fn list(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<RunWorktree>> {
    Ok(sqlx::query_as::<_, WorktreeRow>(
        "SELECT * FROM run_worktrees WHERE workspace_id = ? ORDER BY created_at DESC",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(WorktreeRow::into_model)
    .collect())
}

pub async fn set_status(pool: &SqlitePool, id: &str, status: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE run_worktrees SET status = ?,
            merged_at = CASE WHEN ? = 'MERGED'
                THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') ELSE merged_at END
         WHERE id = ?",
    )
    .bind(status)
    .bind(status)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn all(pool: &SqlitePool) -> AppResult<Vec<RunWorktree>> {
    Ok(
        sqlx::query_as::<_, WorktreeRow>("SELECT * FROM run_worktrees")
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(WorktreeRow::into_model)
            .collect(),
    )
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct SweepReport {
    pub missing_records: usize,
    pub removed_dirs: usize,
}

/// Reconciles app-owned worktree directories at boot:
/// - a row whose path disappeared becomes REMOVED;
/// - a directory under `<data>/worktrees` with no row is deleted.
pub async fn sweep(pool: &SqlitePool, data_dir: &Path) -> AppResult<SweepReport> {
    let rows = all(pool).await?;
    let mut report = SweepReport::default();
    let mut known: HashSet<PathBuf> = HashSet::new();
    for row in &rows {
        let path = PathBuf::from(&row.path);
        known.insert(std::fs::canonicalize(&path).unwrap_or(path.clone()));
        if row.status == "ACTIVE" && !path.exists() {
            set_status(pool, &row.id, "REMOVED").await?;
            report.missing_records += 1;
        }
    }
    let root = data_dir.join("worktrees");
    if root.is_dir() {
        for entry in std::fs::read_dir(&root)? {
            let entry = entry?;
            let path = entry.path();
            let canonical = std::fs::canonicalize(&path).unwrap_or(path.clone());
            if known.contains(&canonical) {
                continue;
            }
            if path.is_dir() {
                std::fs::remove_dir_all(&path)?;
                report.removed_dirs += 1;
            }
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worktree::RunWorktree;

    #[tokio::test]
    async fn sweeps_missing_records_and_unreferenced_dirs() {
        let ctx = crate::test_support::context().await;
        let root = ctx.paths.data_dir.join("worktrees");
        let orphan = root.join("orphan");
        std::fs::create_dir_all(&orphan).expect("orphan");
        let missing = root.join("missing");
        let record = RunWorktree {
            id: "wt-missing".into(),
            workspace_id: ctx.workspace_id.clone(),
            run_id: None,
            path: missing.to_string_lossy().to_string(),
            mode: "overlay".into(),
            branch: None,
            base_commit: None,
            status: "ACTIVE".into(),
            created_at: "2026-11-16T00:00:00Z".into(),
            merged_at: None,
        };
        insert(&ctx.db, &record).await.expect("insert");
        let report = sweep(&ctx.db, &ctx.paths.data_dir).await.expect("sweep");
        assert_eq!(report.missing_records, 1);
        assert_eq!(report.removed_dirs, 1);
        assert!(!orphan.exists());
        assert_eq!(
            get(&ctx.db, "wt-missing").await.expect("get").status,
            "REMOVED"
        );
    }
}
