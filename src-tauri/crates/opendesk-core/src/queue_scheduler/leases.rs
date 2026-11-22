//! Lease timing and shutdown draining for the queue worker.
//!
//! Keeping these helpers out of `queue_manager.rs` prevents the worker loop
//! from growing while it waits on the nearest lease instead of polling.

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::{collections::HashMap, sync::Mutex, time::Duration};
use tokio_util::sync::CancellationToken;

const MAX_WAIT: Duration = Duration::from_secs(30);

/// Time until the earliest running lease expires, capped at 30 seconds.
pub async fn next_lease_wait(pool: &SqlitePool) -> Duration {
    let Ok(Some(deadline)) = crate::db::queue::next_lease_deadline(pool).await else {
        return MAX_WAIT;
    };
    let Ok(deadline) = DateTime::parse_from_rfc3339(&deadline) else {
        return MAX_WAIT;
    };
    (deadline.with_timezone(&Utc) - Utc::now())
        .to_std()
        .unwrap_or(Duration::from_millis(10))
        .min(MAX_WAIT)
}

/// Future used by the worker's `select!` to wake exactly at the next lease.
pub async fn next_wake(pool: &SqlitePool) -> Duration {
    let wait = next_lease_wait(pool).await;
    tokio::time::sleep(wait).await;
    wait
}

/// Bounded reaper: waits up to five seconds for cancelled jobs to unwind so
/// their child processes (OpenCode, CLI harnesses) do not outlive the shell.
pub async fn drain_running(running: &Mutex<HashMap<String, CancellationToken>>) {
    for _ in 0..50 {
        let empty = running
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .is_empty();
        if empty {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(workspace_id: &str) -> crate::db::queue::EnqueueInput {
        crate::db::queue::EnqueueInput {
            workspace_id: workspace_id.into(),
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
    async fn waits_for_the_nearest_lease_and_never_polls() {
        let ctx = crate::test_support::context().await;
        assert_eq!(next_lease_wait(&ctx.db).await, MAX_WAIT);

        let job = crate::db::queue::enqueue(&ctx.db, input(&ctx.workspace_id))
            .await
            .expect("enqueue");
        assert!(
            crate::db::queue::claim(&ctx.db, &job.id)
                .await
                .expect("claim")
        );

        sqlx::query(
            "UPDATE execution_queue
             SET lease_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ','now','+5 seconds')
             WHERE id = ?",
        )
        .bind(&job.id)
        .execute(&ctx.db)
        .await
        .expect("future lease");
        let wait = next_lease_wait(&ctx.db).await;
        assert!(wait <= Duration::from_secs(6), "wait was {wait:?}");

        sqlx::query(
            "UPDATE execution_queue
             SET lease_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ','now','-1 second')
             WHERE id = ?",
        )
        .bind(&job.id)
        .execute(&ctx.db)
        .await
        .expect("expired lease");
        assert_eq!(next_lease_wait(&ctx.db).await, Duration::from_millis(10));
    }

    #[tokio::test]
    async fn drain_returns_immediately_when_no_job_is_running() {
        let running = Mutex::new(HashMap::new());
        let started = std::time::Instant::now();
        drain_running(&running).await;
        assert!(started.elapsed() < Duration::from_millis(100));
    }
}
