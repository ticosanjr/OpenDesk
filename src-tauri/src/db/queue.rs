use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct QueueJob {
    pub id: String,
    pub workspace_id: String,
    pub task_id: Option<String>,
    pub schedule_id: Option<String>,
    pub session_id: Option<String>,
    #[serde(default)]
    pub concurrency_group: Option<String>,
    pub prompt_payload: String,
    pub resolved_mode: String,
    pub resolved_agent: String,
    pub resolved_model: String,
    pub resolved_effort: String,
    #[sqlx(default)]
    pub resolved_harness: String,
    pub job_kind: String,
    pub scheduled_for: Option<String>,
    pub output_dir: Option<String>,
    pub priority: i64,
    pub status: String,
    pub retry_count: i64,
    pub max_retries: i64,
    pub error_message: Option<String>,
    pub enqueued_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    #[sqlx(default)]
    pub lease_expires_at: Option<String>,
    #[sqlx(default)]
    pub last_heartbeat_at: Option<String>,
    #[sqlx(default)]
    pub task_title: Option<String>,
    #[sqlx(default)]
    pub task_origin: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EnqueueInput {
    pub workspace_id: String,
    pub task_id: Option<String>,
    pub schedule_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub concurrency_group: Option<String>,
    pub prompt_payload: String,
    pub resolved_mode: String,
    pub resolved_agent: String,
    pub resolved_model: String,
    pub resolved_effort: String,
    #[serde(default = "default_harness")]
    pub resolved_harness: String,
    #[serde(default = "default_kind")]
    pub job_kind: String,
    pub scheduled_for: Option<String>,
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default = "default_priority")]
    pub priority: i64,
    #[serde(default = "default_max_retries")]
    pub max_retries: i64,
}

fn default_harness() -> String {
    "opencode".into()
}

fn default_kind() -> String {
    "task".into()
}

fn default_priority() -> i64 {
    2
}

fn default_max_retries() -> i64 {
    1
}

const COLUMNS: &str = "q.id, q.workspace_id, q.task_id, q.schedule_id, q.session_id, q.concurrency_group, q.prompt_payload, \
    q.resolved_mode, q.resolved_agent, q.resolved_model, q.resolved_effort, q.resolved_harness, q.job_kind, \
    q.scheduled_for, q.output_dir, q.priority, q.status, q.retry_count, q.max_retries, q.error_message, \
    q.enqueued_at, q.started_at, q.finished_at, t.title AS task_title, t.origin AS task_origin";

fn enqueue_dedupe_key(input: &EnqueueInput) -> String {
    // Prefer stable identity fields over the rendered prompt: the prompt
    // contains an output directory that can change between identical attempts.
    let identity = if let Some(task_id) = &input.task_id {
        format!(
            "task:{task_id}:{}",
            input.session_id.as_deref().unwrap_or("")
        )
    } else if let Some(schedule_id) = &input.schedule_id {
        format!(
            "schedule:{schedule_id}:{}",
            input.scheduled_for.as_deref().unwrap_or("")
        )
    } else {
        input.prompt_payload.clone()
    };
    crate::dedupe::fingerprint(&[
        &input.workspace_id,
        &input.job_kind,
        &input.resolved_mode,
        &input.resolved_agent,
        &input.resolved_model,
        &input.resolved_effort,
        &identity,
    ])
}

async fn active_by_dedupe(
    pool: &SqlitePool,
    workspace_id: &str,
    dedupe_key: &str,
) -> AppResult<Option<QueueJob>> {
    let query = format!(
        "SELECT {COLUMNS} FROM execution_queue q
         LEFT JOIN tasks t ON t.id = q.task_id
         WHERE q.workspace_id = ? AND q.dedupe_key = ?
           AND q.status IN ('QUEUED', 'RUNNING')
         ORDER BY q.enqueued_at DESC LIMIT 1"
    );
    Ok(sqlx::query_as::<_, QueueJob>(&query)
        .bind(workspace_id)
        .bind(dedupe_key)
        .fetch_optional(pool)
        .await?)
}

pub async fn enqueue(pool: &SqlitePool, input: EnqueueInput) -> AppResult<QueueJob> {
    let dedupe_key = enqueue_dedupe_key(&input);
    if let Some(existing) = active_by_dedupe(pool, &input.workspace_id, &dedupe_key).await? {
        return Ok(existing);
    }
    let id = Uuid::now_v7().to_string();
    let insert = sqlx::query(
        "INSERT INTO execution_queue (
            id, workspace_id, task_id, schedule_id, session_id, concurrency_group,
            prompt_payload, resolved_mode, resolved_agent, resolved_model, resolved_effort,
            resolved_harness, job_kind, scheduled_for, output_dir, priority, max_retries, dedupe_key
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.workspace_id)
    .bind(input.task_id.as_deref())
    .bind(input.schedule_id.as_deref())
    .bind(input.session_id.as_deref())
    .bind(input.concurrency_group.as_deref().unwrap_or("default"))
    .bind(&input.prompt_payload)
    .bind(&input.resolved_mode)
    .bind(&input.resolved_agent)
    .bind(&input.resolved_model)
    .bind(&input.resolved_effort)
    .bind(&input.resolved_harness)
    .bind(&input.job_kind)
    .bind(input.scheduled_for.as_deref())
    .bind(input.output_dir.as_deref())
    .bind(input.priority.clamp(1, 4))
    .bind(input.max_retries.max(0))
    .bind(&dedupe_key)
    .execute(pool)
    .await;

    if let Err(error) = insert {
        if let sqlx::Error::Database(database) = &error
            && database.message().contains("UNIQUE constraint failed")
            && let Some(existing) = active_by_dedupe(pool, &input.workspace_id, &dedupe_key).await?
        {
            return Ok(existing);
        }
        return Err(error.into());
    }

    get(pool, &id).await
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<QueueJob> {
    let query = format!(
        "SELECT {COLUMNS} FROM execution_queue q
         LEFT JOIN tasks t ON t.id = q.task_id
         WHERE q.id = ?"
    );
    sqlx::query_as::<_, QueueJob>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("queue job {id}")))
}

pub async fn list(pool: &SqlitePool, workspace_id: Option<&str>) -> AppResult<Vec<QueueJob>> {
    let query = format!(
        "SELECT {COLUMNS} FROM execution_queue q
         LEFT JOIN tasks t ON t.id = q.task_id
         WHERE (? IS NULL OR q.workspace_id = ?)
         ORDER BY CASE q.status
            WHEN 'RUNNING' THEN 0 WHEN 'QUEUED' THEN 1 WHEN 'PAUSED' THEN 2
            WHEN 'FAILED' THEN 3 ELSE 4 END,
            q.priority DESC, q.enqueued_at ASC
         LIMIT 200"
    );
    Ok(sqlx::query_as::<_, QueueJob>(&query)
        .bind(workspace_id)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn next_queued(pool: &SqlitePool) -> AppResult<Option<QueueJob>> {
    let query = format!(
        "SELECT {COLUMNS} FROM execution_queue q
         LEFT JOIN tasks t ON t.id = q.task_id
         WHERE q.status = 'QUEUED'
           AND (
             SELECT COUNT(*) FROM execution_queue r
             WHERE r.status = 'RUNNING' AND r.concurrency_group = q.concurrency_group
           ) < CASE q.concurrency_group
             WHEN 'local_llm_heavy' THEN 1
             WHEN 'web_scraper' THEN 2
             WHEN 'light_api' THEN 5
             ELSE 100
           END
           -- Optional per-agent quota. No profile means the group limit above
           -- remains the only constraint.
           AND (
             NOT EXISTS (
               SELECT 1 FROM agent_profiles p
               WHERE p.workspace_id = q.workspace_id
                 AND p.agent_id = q.resolved_agent
                 AND p.concurrency_quota > 0
             )
             OR (
               SELECT COUNT(*) FROM execution_queue r
               WHERE r.status = 'RUNNING'
                 AND r.workspace_id = q.workspace_id
                 AND r.resolved_agent = q.resolved_agent
             ) < (
               SELECT p.concurrency_quota FROM agent_profiles p
               WHERE p.workspace_id = q.workspace_id AND p.agent_id = q.resolved_agent
             )
           )
         ORDER BY q.priority DESC, q.enqueued_at ASC
         LIMIT 1"
    );
    Ok(sqlx::query_as::<_, QueueJob>(&query)
        .fetch_optional(pool)
        .await?)
}

pub async fn claim(pool: &SqlitePool, id: &str) -> AppResult<bool> {
    let result = sqlx::query(
        "UPDATE execution_queue SET status = 'RUNNING',
            started_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            error_message = NULL
         WHERE id = ? AND status = 'QUEUED'",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn heartbeat(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE execution_queue SET last_heartbeat_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            lease_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+60 seconds')
         WHERE id = ? AND status = 'RUNNING'",
    )
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn expired_leases(pool: &SqlitePool) -> AppResult<Vec<QueueJob>> {
    let query = format!(
        "SELECT {COLUMNS} FROM execution_queue q
         LEFT JOIN tasks t ON t.id = q.task_id
         WHERE q.status = 'RUNNING' AND q.lease_expires_at IS NOT NULL
           AND q.lease_expires_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"
    );
    Ok(sqlx::query_as::<_, QueueJob>(&query)
        .fetch_all(pool)
        .await?)
}

pub async fn finish(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    error_message: Option<&str>,
) -> AppResult<()> {
    if !matches!(status, "COMPLETED" | "FAILED" | "CANCELLED" | "PAUSED") {
        return Err(AppError::BadRequest(format!(
            "invalid queue status '{status}'"
        )));
    }
    sqlx::query(
        "UPDATE execution_queue SET status = ?, error_message = ?,
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(status)
    .bind(error_message)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

#[allow(dead_code)]
pub async fn set_status(pool: &SqlitePool, id: &str, status: &str) -> AppResult<()> {
    sqlx::query("UPDATE execution_queue SET status = ? WHERE id = ?")
        .bind(status)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn cancel(pool: &SqlitePool, id: &str) -> AppResult<QueueJob> {
    let job = get(pool, id).await?;
    if job.status == "RUNNING" {
        return Err(AppError::Conflict(
            "running jobs must be cancelled through the running-job endpoint".into(),
        ));
    }
    if !matches!(job.status.as_str(), "QUEUED" | "PAUSED") {
        return Err(AppError::Conflict(format!(
            "cannot cancel a job in status {}",
            job.status
        )));
    }
    sqlx::query(
        "UPDATE execution_queue SET status = 'CANCELLED',
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn retry(pool: &SqlitePool, id: &str) -> AppResult<QueueJob> {
    let job = get(pool, id).await?;
    if !matches!(job.status.as_str(), "FAILED" | "CANCELLED") {
        return Err(AppError::Conflict(format!(
            "cannot retry a job in status {}",
            job.status
        )));
    }
    if job.retry_count >= job.max_retries {
        return Err(AppError::Conflict(
            "this job reached its max_retries limit".into(),
        ));
    }
    sqlx::query(
        "UPDATE execution_queue SET status = 'QUEUED',
            retry_count = retry_count + 1, error_message = NULL,
            started_at = NULL, finished_at = NULL
         WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}

pub async fn update_priority(pool: &SqlitePool, id: &str, priority: i64) -> AppResult<QueueJob> {
    if !(1..=4).contains(&priority) {
        return Err(AppError::BadRequest("priority must be 1..=4".into()));
    }
    sqlx::query("UPDATE execution_queue SET priority = ? WHERE id = ?")
        .bind(priority)
        .bind(id)
        .execute(pool)
        .await?;
    get(pool, id).await
}

pub async fn recover_running(pool: &SqlitePool) -> AppResult<u64> {
    let result = sqlx::query(
        "UPDATE execution_queue SET status = 'FAILED',
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            error_message = 'interrupted by OpenDesk restart; manual retry required'
         WHERE status = 'RUNNING'",
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn counts(pool: &SqlitePool, workspace_id: Option<&str>) -> AppResult<(i64, i64)> {
    let row: (i64, i64) = sqlx::query_as(
        "SELECT
            COALESCE(SUM(CASE WHEN status = 'RUNNING' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN status IN ('QUEUED', 'PAUSED') THEN 1 ELSE 0 END), 0)
         FROM execution_queue
         WHERE (? IS NULL OR workspace_id = ?)",
    )
    .bind(workspace_id)
    .bind(workspace_id)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("queue.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        (temp, pool)
    }

    fn input() -> EnqueueInput {
        EnqueueInput {
            workspace_id: "ws-1".into(),
            task_id: None,
            schedule_id: None,
            session_id: None,
            concurrency_group: None,
            prompt_payload: "prompt".into(),
            resolved_mode: "build".into(),
            resolved_agent: "build".into(),
            resolved_model: "opencode-go/glm-5.3".into(),
            resolved_effort: "high".into(),
            resolved_harness: "opencode".into(),
            job_kind: "task".into(),
            scheduled_for: None,
            output_dir: None,
            priority: 2,
            max_retries: 1,
        }
    }

    #[tokio::test]
    async fn enqueue_reuses_identical_active_jobs() {
        let (_temp, pool) = setup().await;
        let first = enqueue(&pool, input()).await.expect("first");
        let duplicate = enqueue(&pool, input()).await.expect("duplicate");
        assert_eq!(duplicate.id, first.id);

        let mut other = input();
        other.resolved_effort = "low".into();
        let different = enqueue(&pool, other).await.expect("different");
        assert_ne!(different.id, first.id);

        finish(&pool, &first.id, "COMPLETED", None)
            .await
            .expect("finish");
        let after = enqueue(&pool, input()).await.expect("after");
        assert_ne!(after.id, first.id);
    }

    #[tokio::test]
    async fn queue_lifecycle_and_recovery() {
        let (_temp, pool) = setup().await;
        let job = enqueue(&pool, input()).await.expect("enqueue");
        assert_eq!(job.status, "QUEUED");
        assert_eq!(next_queued(&pool).await.expect("next").unwrap().id, job.id);

        assert!(claim(&pool, &job.id).await.expect("claim"));
        assert_eq!(get(&pool, &job.id).await.expect("get").status, "RUNNING");

        finish(&pool, &job.id, "FAILED", Some("boom"))
            .await
            .expect("finish");
        let retried = retry(&pool, &job.id).await.expect("retry");
        assert_eq!(retried.status, "QUEUED");
        assert_eq!(retried.retry_count, 1);

        assert!(claim(&pool, &job.id).await.expect("claim 2"));
        assert_eq!(recover_running(&pool).await.expect("recover"), 1);
        assert_eq!(get(&pool, &job.id).await.expect("get 2").status, "FAILED");
    }

    #[tokio::test]
    async fn concurrency_group_blocks_second_heavy_job() {
        let (_temp, pool) = setup().await;
        let mut heavy = input();
        heavy.concurrency_group = Some("local_llm_heavy".into());
        let first = enqueue(&pool, heavy.clone()).await.expect("first");
        enqueue(&pool, heavy).await.expect("second");
        assert!(claim(&pool, &first.id).await.expect("claim"));
        assert!(next_queued(&pool).await.expect("next").is_none());
    }

    #[tokio::test]
    async fn heartbeat_and_expired_leases() {
        let (_temp, pool) = setup().await;
        let job = enqueue(&pool, input()).await.expect("enqueue");
        assert!(claim(&pool, &job.id).await.expect("claim"));
        heartbeat(&pool, &job.id).await.expect("heartbeat");
        assert!(expired_leases(&pool).await.expect("leases").is_empty());
        sqlx::query(
            "UPDATE execution_queue SET lease_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ','now','-1 second') WHERE id = ?",
        )
        .bind(&job.id)
        .execute(&pool)
        .await
        .expect("expire");
        let expired = expired_leases(&pool).await.expect("expired");
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].id, job.id);
    }

    #[tokio::test]
    async fn cancel_and_priority() {
        let (_temp, pool) = setup().await;
        let job = enqueue(&pool, input()).await.expect("enqueue");
        let updated = update_priority(&pool, &job.id, 4).await.expect("priority");
        assert_eq!(updated.priority, 4);
        let cancelled = cancel(&pool, &job.id).await.expect("cancel");
        assert_eq!(cancelled.status, "CANCELLED");
        assert!(cancel(&pool, &job.id).await.is_err());
    }
}
