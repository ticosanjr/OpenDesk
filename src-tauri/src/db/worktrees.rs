use crate::{
    error::{AppError, AppResult},
    worktree::RunWorktree,
};
use sqlx::{FromRow, SqlitePool};

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
