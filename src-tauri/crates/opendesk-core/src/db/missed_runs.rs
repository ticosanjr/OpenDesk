use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MissedRun {
    pub id: String,
    pub workspace_id: String,
    pub schedule_id: String,
    pub scheduled_for: String,
    pub detected_at: String,
    pub reason: String,
    pub status: String,
    pub run_id: Option<String>,
    pub error_message: Option<String>,
    #[sqlx(default)]
    pub schedule_name: Option<String>,
}

const COLUMNS: &str = "id, workspace_id, schedule_id, scheduled_for, detected_at, \
    reason, status, run_id, error_message";

pub async fn create_or_ignore(
    pool: &SqlitePool,
    workspace_id: &str,
    schedule_id: &str,
    scheduled_for: &str,
    reason: &str,
) -> AppResult<MissedRun> {
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT OR IGNORE INTO missed_runs (
            id, workspace_id, schedule_id, scheduled_for, reason
         ) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(schedule_id)
    .bind(scheduled_for)
    .bind(reason)
    .execute(pool)
    .await?;
    let query =
        format!("SELECT {COLUMNS} FROM missed_runs WHERE schedule_id = ? AND scheduled_for = ?");
    sqlx::query_as::<_, MissedRun>(&query)
        .bind(schedule_id)
        .bind(scheduled_for)
        .fetch_one(pool)
        .await
        .map_err(AppError::from)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<MissedRun> {
    let query = format!("SELECT {COLUMNS} FROM missed_runs WHERE id = ?");
    sqlx::query_as::<_, MissedRun>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("missed run {id}")))
}

pub async fn list_pending(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<MissedRun>> {
    let query = format!(
        "SELECT {COLUMNS} FROM missed_runs
         WHERE workspace_id = ? AND status = 'PENDING'
         ORDER BY scheduled_for ASC"
    );
    Ok(sqlx::query_as::<_, MissedRun>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn list_pending_with_names(
    pool: &SqlitePool,
    workspace_id: &str,
) -> AppResult<Vec<MissedRun>> {
    let query = "SELECT m.id, m.workspace_id, m.schedule_id, m.scheduled_for, m.detected_at, \
         m.reason, m.status, m.run_id, m.error_message, s.name AS schedule_name \
         FROM missed_runs m JOIN schedules s ON s.id = m.schedule_id \
         WHERE m.workspace_id = ? AND m.status = 'PENDING' \
         ORDER BY m.scheduled_for ASC";
    Ok(sqlx::query_as::<_, MissedRun>(query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn list_for_workspace(
    pool: &SqlitePool,
    workspace_id: &str,
    status: Option<&str>,
) -> AppResult<Vec<MissedRun>> {
    let query = format!(
        "SELECT {COLUMNS} FROM missed_runs
         WHERE workspace_id = ? AND (? IS NULL OR status = ?)
         ORDER BY scheduled_for DESC LIMIT 200"
    );
    Ok(sqlx::query_as::<_, MissedRun>(&query)
        .bind(workspace_id)
        .bind(status)
        .bind(status)
        .fetch_all(pool)
        .await?)
}

pub async fn list_range(
    pool: &SqlitePool,
    workspace_id: &str,
    from: &str,
    to: &str,
) -> AppResult<Vec<MissedRun>> {
    let query = format!(
        "SELECT {COLUMNS} FROM missed_runs
         WHERE workspace_id = ? AND scheduled_for >= ? AND scheduled_for <= ?
         ORDER BY scheduled_for ASC"
    );
    Ok(sqlx::query_as::<_, MissedRun>(&query)
        .bind(workspace_id)
        .bind(from)
        .bind(to)
        .fetch_all(pool)
        .await?)
}

pub async fn set_status(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    run_id: Option<&str>,
    error_message: Option<&str>,
) -> AppResult<MissedRun> {
    if !matches!(
        status,
        "PENDING" | "QUEUED" | "IGNORED" | "FAILED" | "CANCELLED"
    ) {
        return Err(AppError::BadRequest(format!(
            "invalid missed run status '{status}'"
        )));
    }
    sqlx::query(
        "UPDATE missed_runs SET status = ?, run_id = ?, error_message = ?
         WHERE id = ?",
    )
    .bind(status)
    .bind(run_id)
    .bind(error_message)
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}
