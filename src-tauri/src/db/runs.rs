use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskRun {
    pub id: String,
    pub queue_job_id: Option<String>,
    pub workspace_id: String,
    pub task_id: Option<String>,
    pub parent_run_id: Option<String>,
    pub phase: Option<String>,
    pub agent_name: String,
    pub execution_mode: String,
    pub provider_id: Option<String>,
    pub model_id: String,
    pub effort_level: String,
    pub status: String,
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
    pub tokens_cache_read: i64,
    pub tokens_cache_write: i64,
    pub estimated_cost_usd: f64,
    pub pricing_snapshot_json: Option<String>,
    pub duration_ms: i64,
    pub log_path: Option<String>,
    pub raw_stdout_log: Option<String>,
    pub summary_response: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
    #[sqlx(default)]
    pub task_title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateRunInput {
    pub id: String,
    pub queue_job_id: Option<String>,
    pub workspace_id: String,
    pub task_id: Option<String>,
    pub parent_run_id: Option<String>,
    pub phase: Option<String>,
    pub agent_name: String,
    pub execution_mode: String,
    pub provider_id: Option<String>,
    pub model_id: String,
    pub effort_level: String,
    pub pricing_snapshot_json: Option<String>,
    pub log_path: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct RunUsage {
    pub tokens_input: i64,
    pub tokens_output: i64,
    pub tokens_reasoning: i64,
    pub tokens_cache_read: i64,
    pub tokens_cache_write: i64,
    pub estimated_cost_usd: f64,
}

const COLUMNS: &str = "r.id, r.queue_job_id, r.workspace_id, r.task_id, r.parent_run_id, r.phase, \
    r.agent_name, r.execution_mode, r.provider_id, r.model_id, r.effort_level, r.status, \
    r.tokens_input, r.tokens_output, r.tokens_reasoning, r.tokens_cache_read, r.tokens_cache_write, \
    r.estimated_cost_usd, r.pricing_snapshot_json, r.duration_ms, r.log_path, r.raw_stdout_log, \
    r.summary_response, r.started_at, r.completed_at, t.title AS task_title";

pub async fn create(pool: &SqlitePool, input: CreateRunInput) -> AppResult<TaskRun> {
    let id = input.id;
    sqlx::query(
        "INSERT INTO task_runs (
            id, queue_job_id, workspace_id, task_id, parent_run_id, phase,
            agent_name, execution_mode, provider_id, model_id, effort_level,
            status, pricing_snapshot_json, log_path
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'RUNNING', ?, ?)",
    )
    .bind(&id)
    .bind(input.queue_job_id.as_deref())
    .bind(&input.workspace_id)
    .bind(input.task_id.as_deref())
    .bind(input.parent_run_id.as_deref())
    .bind(input.phase.as_deref())
    .bind(&input.agent_name)
    .bind(&input.execution_mode)
    .bind(input.provider_id.as_deref())
    .bind(&input.model_id)
    .bind(&input.effort_level)
    .bind(input.pricing_snapshot_json.as_deref())
    .bind(input.log_path.as_deref())
    .execute(pool)
    .await?;

    get(pool, &id).await
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<TaskRun> {
    let query = format!(
        "SELECT {COLUMNS} FROM task_runs r LEFT JOIN tasks t ON t.id = r.task_id WHERE r.id = ?"
    );
    sqlx::query_as::<_, TaskRun>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("run {id}")))
}

pub async fn list(
    pool: &SqlitePool,
    workspace_id: &str,
    task_id: Option<&str>,
) -> AppResult<Vec<TaskRun>> {
    let query = format!(
        "SELECT {COLUMNS} FROM task_runs r
         LEFT JOIN tasks t ON t.id = r.task_id
         WHERE r.workspace_id = ? AND (? IS NULL OR r.task_id = ?)
         ORDER BY r.started_at DESC LIMIT 100"
    );
    Ok(sqlx::query_as::<_, TaskRun>(&query)
        .bind(workspace_id)
        .bind(task_id)
        .bind(task_id)
        .fetch_all(pool)
        .await?)
}

pub async fn finish(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    usage: &RunUsage,
    duration_ms: i64,
    summary: Option<&str>,
    log_tail: Option<&str>,
) -> AppResult<()> {
    if !matches!(status, "SUCCESS" | "FAILED" | "CANCELLED") {
        return Err(AppError::BadRequest(format!(
            "invalid run status '{status}'"
        )));
    }
    sqlx::query(
        "UPDATE task_runs SET status = ?, tokens_input = ?, tokens_output = ?,
            tokens_reasoning = ?, tokens_cache_read = ?, tokens_cache_write = ?,
            estimated_cost_usd = ?, duration_ms = ?, summary_response = ?,
            raw_stdout_log = ?,
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(status)
    .bind(usage.tokens_input)
    .bind(usage.tokens_output)
    .bind(usage.tokens_reasoning)
    .bind(usage.tokens_cache_read)
    .bind(usage.tokens_cache_write)
    .bind(usage.estimated_cost_usd)
    .bind(duration_ms)
    .bind(summary)
    .bind(log_tail)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_log_path(pool: &SqlitePool, id: &str, log_path: &str) -> AppResult<()> {
    sqlx::query("UPDATE task_runs SET log_path = ? WHERE id = ?")
        .bind(log_path)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn recover_running(pool: &SqlitePool) -> AppResult<u64> {
    let result = sqlx::query(
        "UPDATE task_runs SET status = 'FAILED',
            summary_response = 'interrupted after OpenDesk restart',
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE status = 'RUNNING'",
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn count_for_task(pool: &SqlitePool, task_id: &str) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM task_runs WHERE task_id = ?")
            .bind(task_id)
            .fetch_one(pool)
            .await?,
    )
}
