use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TaskRun {
    pub id: String,
    pub queue_job_id: Option<String>,
    pub workspace_id: String,
    pub task_id: Option<String>,
    pub schedule_id: Option<String>,
    pub attempt_number: i64,
    pub session_id: Option<String>,
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
    pub error_code: Option<String>,
    #[sqlx(default)]
    pub error_message: Option<String>,
    #[sqlx(default)]
    pub error_hint: Option<String>,
    #[sqlx(default)]
    pub task_title: Option<String>,
    #[sqlx(default)]
    pub task_origin: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateRunInput {
    pub id: String,
    pub queue_job_id: Option<String>,
    pub workspace_id: String,
    pub task_id: Option<String>,
    pub schedule_id: Option<String>,
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

const COLUMNS: &str = "r.id, r.queue_job_id, r.workspace_id, r.task_id, r.schedule_id, r.attempt_number, r.session_id, r.parent_run_id, r.phase, \
    r.agent_name, r.execution_mode, r.provider_id, r.model_id, r.effort_level, r.status, \
    r.tokens_input, r.tokens_output, r.tokens_reasoning, r.tokens_cache_read, r.tokens_cache_write, \
    r.estimated_cost_usd, r.pricing_snapshot_json, r.duration_ms, r.log_path, r.raw_stdout_log, \
    r.summary_response, r.started_at, r.completed_at, r.error_code, r.error_message, \
    r.error_hint, t.title AS task_title, t.origin AS task_origin";

pub async fn create(pool: &SqlitePool, input: CreateRunInput) -> AppResult<TaskRun> {
    let id = input.id;
    let attempt_number: i64 = match input.task_id.as_deref() {
        Some(task_id) => {
            sqlx::query_scalar(
                "SELECT COALESCE(MAX(attempt_number), 0) + 1 FROM task_runs WHERE task_id = ?",
            )
            .bind(task_id)
            .fetch_one(pool)
            .await?
        }
        None => 1,
    };
    sqlx::query(
        "INSERT INTO task_runs (
            id, queue_job_id, workspace_id, task_id, schedule_id, attempt_number,
            parent_run_id, phase, agent_name, execution_mode, provider_id, model_id,
            effort_level, status, pricing_snapshot_json, log_path
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'RUNNING', ?, ?)",
    )
    .bind(&id)
    .bind(input.queue_job_id.as_deref())
    .bind(&input.workspace_id)
    .bind(input.task_id.as_deref())
    .bind(input.schedule_id.as_deref())
    .bind(attempt_number)
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
    list_with_limit(pool, workspace_id, task_id, 100).await
}

pub async fn list_with_limit(
    pool: &SqlitePool,
    workspace_id: &str,
    task_id: Option<&str>,
    limit: i64,
) -> AppResult<Vec<TaskRun>> {
    let query = format!(
        "SELECT {COLUMNS} FROM task_runs r
         LEFT JOIN tasks t ON t.id = r.task_id
         WHERE r.workspace_id = ? AND (? IS NULL OR r.task_id = ?)
         ORDER BY r.started_at DESC LIMIT ?"
    );
    Ok(sqlx::query_as::<_, TaskRun>(&query)
        .bind(workspace_id)
        .bind(task_id)
        .bind(task_id)
        .bind(limit.clamp(1, 1000))
        .fetch_all(pool)
        .await?)
}

pub async fn latest_for_task(pool: &SqlitePool, task_id: &str) -> AppResult<Option<TaskRun>> {
    let query = format!(
        "SELECT {COLUMNS} FROM task_runs r LEFT JOIN tasks t ON t.id = r.task_id
         WHERE r.task_id = ? ORDER BY r.started_at DESC, r.attempt_number DESC LIMIT 1"
    );
    Ok(sqlx::query_as::<_, TaskRun>(&query)
        .bind(task_id)
        .fetch_optional(pool)
        .await?)
}

pub async fn list_by_schedule(pool: &SqlitePool, schedule_id: &str) -> AppResult<Vec<TaskRun>> {
    let query = format!(
        "SELECT {COLUMNS} FROM task_runs r
         LEFT JOIN tasks t ON t.id = r.task_id
         WHERE r.schedule_id = ?
         ORDER BY r.started_at DESC LIMIT 100"
    );
    Ok(sqlx::query_as::<_, TaskRun>(&query)
        .bind(schedule_id)
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

#[allow(dead_code)]
pub async fn set_log_path(pool: &SqlitePool, id: &str, log_path: &str) -> AppResult<()> {
    sqlx::query("UPDATE task_runs SET log_path = ? WHERE id = ?")
        .bind(log_path)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_error(
    pool: &SqlitePool,
    id: &str,
    code: &str,
    message: &str,
    hint: &str,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE task_runs SET error_code = ?, error_message = ?, error_hint = ? WHERE id = ?",
    )
    .bind(code)
    .bind(message)
    .bind(hint)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

#[allow(dead_code)]
pub async fn set_schedule_id(pool: &SqlitePool, id: &str, schedule_id: &str) -> AppResult<()> {
    sqlx::query("UPDATE task_runs SET schedule_id = ? WHERE id = ?")
        .bind(schedule_id)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn set_session(pool: &SqlitePool, id: &str, session_id: Option<&str>) -> AppResult<()> {
    sqlx::query("UPDATE task_runs SET session_id = ? WHERE id = ?")
        .bind(session_id)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn latest_session_for_task(
    pool: &SqlitePool,
    task_id: &str,
) -> AppResult<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT session_id FROM task_runs
         WHERE task_id = ? AND session_id IS NOT NULL AND session_id != ''
         ORDER BY started_at DESC LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await?
    .flatten())
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

#[allow(dead_code)]
pub async fn count_for_task(pool: &SqlitePool, task_id: &str) -> AppResult<i64> {
    Ok(
        sqlx::query_scalar("SELECT COUNT(*) FROM task_runs WHERE task_id = ?")
            .bind(task_id)
            .fetch_one(pool)
            .await?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("runs.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        (temp, pool)
    }

    fn input(id: &str) -> CreateRunInput {
        CreateRunInput {
            id: id.into(),
            queue_job_id: None,
            workspace_id: "ws-1".into(),
            task_id: None,
            schedule_id: None,
            parent_run_id: None,
            phase: Some("build".into()),
            agent_name: "build".into(),
            execution_mode: "build".into(),
            provider_id: Some("opencode-go".into()),
            model_id: "glm-5.3".into(),
            effort_level: "high".into(),
            pricing_snapshot_json: None,
            log_path: Some("/tmp/run.jsonl".into()),
        }
    }

    #[tokio::test]
    async fn run_lifecycle_and_recovery() {
        let (_temp, pool) = setup().await;
        let run = create(&pool, input("run-1")).await.expect("create");
        assert_eq!(run.status, "RUNNING");
        assert_eq!(get(&pool, "run-1").await.expect("get").agent_name, "build");

        let usage = RunUsage {
            tokens_input: 10,
            tokens_output: 20,
            tokens_reasoning: 5,
            tokens_cache_read: 1,
            tokens_cache_write: 2,
            estimated_cost_usd: 0.03,
        };
        finish(
            &pool,
            "run-1",
            "SUCCESS",
            &usage,
            1500,
            Some("resumo"),
            Some("tail"),
        )
        .await
        .expect("finish");
        let done = get(&pool, "run-1").await.expect("get");
        assert_eq!(done.status, "SUCCESS");
        assert_eq!(done.tokens_output, 20);
        assert_eq!(done.duration_ms, 1500);
        assert!(done.completed_at.is_some());
        set_session(&pool, "run-1", Some("ses-1"))
            .await
            .expect("session");
        assert_eq!(
            get(&pool, "run-1")
                .await
                .expect("get")
                .session_id
                .as_deref(),
            Some("ses-1")
        );

        create(&pool, input("run-2")).await.expect("create 2");
        assert_eq!(recover_running(&pool).await.expect("recover"), 1);
        assert_eq!(get(&pool, "run-2").await.expect("get 2").status, "FAILED");

        set_log_path(&pool, "run-1", "/tmp/new.jsonl")
            .await
            .expect("log");
        set_error(&pool, "run-1", "timeout", "timed out", "hint")
            .await
            .expect("error");
        let errored = get(&pool, "run-1").await.expect("get");
        assert_eq!(errored.error_code.as_deref(), Some("timeout"));
        assert_eq!(errored.error_message.as_deref(), Some("timed out"));
        assert_eq!(errored.error_hint.as_deref(), Some("hint"));
        assert_eq!(list(&pool, "ws-1", None).await.expect("list").len(), 2);
        assert_eq!(count_for_task(&pool, "none").await.expect("count"), 0);
        assert!(
            finish(&pool, "run-1", "UNKNOWN", &usage, 0, None, None)
                .await
                .is_err()
        );
    }
}
