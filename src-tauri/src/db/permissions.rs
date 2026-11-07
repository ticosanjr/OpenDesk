use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use tokio::time::{Duration, sleep};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PermissionRequest {
    pub id: String,
    pub run_id: Option<String>,
    pub task_id: Option<String>,
    pub workspace_id: String,
    pub session_id: String,
    pub request_id: String,
    pub permission_action: String,
    pub resources: Option<String>,
    pub status: String,
    pub decision: Option<String>,
    pub created_at: String,
    pub decided_at: Option<String>,
}

const COLUMNS: &str = "id, run_id, task_id, workspace_id, session_id, request_id, \
    permission_action, resources, status, decision, created_at, decided_at";

#[allow(clippy::too_many_arguments)]
pub async fn create(
    pool: &SqlitePool,
    run_id: Option<&str>,
    task_id: Option<&str>,
    workspace_id: &str,
    session_id: &str,
    request_id: &str,
    action: &str,
    resources: &[String],
) -> AppResult<PermissionRequest> {
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO permission_requests (
            id, run_id, task_id, workspace_id, session_id, request_id,
            permission_action, resources
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(run_id)
    .bind(task_id)
    .bind(workspace_id)
    .bind(session_id)
    .bind(request_id)
    .bind(action)
    .bind(resources.join(","))
    .execute(pool)
    .await?;
    get_by_request(pool, request_id).await
}

pub async fn get_by_request(pool: &SqlitePool, request_id: &str) -> AppResult<PermissionRequest> {
    let query = format!("SELECT {COLUMNS} FROM permission_requests WHERE request_id = ?");
    sqlx::query_as::<_, PermissionRequest>(&query)
        .bind(request_id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("permission request {request_id}")))
}

pub async fn pending(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<PermissionRequest>> {
    let query = format!(
        "SELECT {COLUMNS} FROM permission_requests
         WHERE workspace_id = ? AND status = 'PENDING'
         ORDER BY created_at ASC"
    );
    Ok(sqlx::query_as::<_, PermissionRequest>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn decide(
    pool: &SqlitePool,
    request_id: &str,
    decision: &str,
) -> AppResult<PermissionRequest> {
    if !matches!(decision, "once" | "always" | "reject") {
        return Err(AppError::BadRequest(format!(
            "decisão inválida '{decision}'; use once, always ou reject"
        )));
    }
    sqlx::query(
        "UPDATE permission_requests
         SET status = 'DECIDED', decision = ?,
             decided_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE request_id = ? AND status = 'PENDING'",
    )
    .bind(decision)
    .bind(request_id)
    .execute(pool)
    .await?;
    get_by_request(pool, request_id).await
}

/// Resolve a pending request by a short prefix, used by chat/Signal replies.
pub async fn decide_prefix(
    pool: &SqlitePool,
    prefix: &str,
    decision: &str,
) -> AppResult<Option<PermissionRequest>> {
    let prefix = prefix.trim();
    if prefix.len() < 4 {
        return Ok(None);
    }
    let request_id = sqlx::query_scalar::<_, String>(
        "SELECT request_id FROM permission_requests
         WHERE status = 'PENDING' AND request_id LIKE ?
         ORDER BY created_at ASC LIMIT 1",
    )
    .bind(format!("{prefix}%"))
    .fetch_optional(pool)
    .await?;
    match request_id {
        Some(request_id) => decide(pool, &request_id, decision).await.map(Some),
        None => Ok(None),
    }
}

/// Block the run until a human replies, the run is cancelled, or the request
/// expires. Returns the decision (`once`, `always`, `reject`) or `None`.
pub async fn wait_for_decision(
    pool: &SqlitePool,
    request_id: &str,
    cancel: &CancellationToken,
    timeout: Duration,
) -> Option<String> {
    let started = std::time::Instant::now();
    loop {
        if cancel.is_cancelled() {
            return None;
        }
        if let Ok(request) = get_by_request(pool, request_id).await
            && request.status == "DECIDED"
        {
            return request.decision;
        }
        if started.elapsed() >= timeout {
            let _ = sqlx::query(
                "UPDATE permission_requests SET status = 'EXPIRED'
                 WHERE request_id = ? AND status = 'PENDING'",
            )
            .bind(request_id)
            .execute(pool)
            .await;
            return None;
        }
        tokio::select! {
            _ = cancel.cancelled() => return None,
            _ = sleep(Duration::from_millis(400)) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (tempfile::TempDir, SqlitePool) {
        let temp = tempfile::TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("permissions.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        (temp, pool)
    }

    #[tokio::test]
    async fn decision_unblocks_waiter() {
        let (_temp, pool) = setup().await;
        let request = create(
            &pool,
            None,
            None,
            "ws-1",
            "ses-1",
            "req-1",
            "bash",
            &["rm -rf build".into()],
        )
        .await
        .expect("create");
        assert_eq!(request.status, "PENDING");
        assert_eq!(pending(&pool, "ws-1").await.expect("pending").len(), 1);

        let pool_for_waiter = pool.clone();
        let waiter = tokio::spawn(async move {
            wait_for_decision(
                &pool_for_waiter,
                "req-1",
                &CancellationToken::new(),
                Duration::from_secs(5),
            )
            .await
        });
        sleep(Duration::from_millis(50)).await;
        decide(&pool, "req-1", "always").await.expect("decide");
        assert_eq!(waiter.await.expect("join"), Some("always".into()));
        assert!(pending(&pool, "ws-1").await.expect("pending").is_empty());
    }

    #[tokio::test]
    async fn short_prefix_resolves_pending_request() {
        let (_temp, pool) = setup().await;
        create(
            &pool,
            None,
            None,
            "ws-1",
            "ses-1",
            "req_abcdef123456",
            "bash",
            &[],
        )
        .await
        .expect("create");
        let decided = decide_prefix(&pool, "req_abc", "once")
            .await
            .expect("decide")
            .expect("found");
        assert_eq!(decided.decision.as_deref(), Some("once"));
        assert!(
            decide_prefix(&pool, "req_abc", "once")
                .await
                .expect("second")
                .is_none()
        );
    }

    #[tokio::test]
    async fn cancel_returns_none() {
        let (_temp, pool) = setup().await;
        create(&pool, None, None, "ws-1", "ses-1", "req-2", "edit", &[])
            .await
            .expect("create");
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert_eq!(
            wait_for_decision(&pool, "req-2", &cancel, Duration::from_secs(1)).await,
            None
        );
    }
}
