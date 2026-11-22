//! Moved to `opendesk-storage`; the re-export keeps the V1 path alive.
pub use opendesk_storage::write_buffer::{BufferedWrite, WriteBuffer, WriteBufferMetrics};

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;
    use std::time::Duration;

    async fn running_job(pool: &SqlitePool, workspace_id: &str, index: usize) -> String {
        let job = crate::db::queue::enqueue(
            pool,
            crate::db::queue::EnqueueInput {
                workspace_id: workspace_id.into(),
                task_id: None,
                schedule_id: None,
                session_id: None,
                concurrency_group: None,
                prompt_payload: format!("buffered job {index}"),
                resolved_mode: "build".into(),
                resolved_agent: "agent".into(),
                resolved_model: "provider/model".into(),
                resolved_effort: "medium".into(),
                resolved_harness: "opencode".into(),
                job_kind: "task".into(),
                scheduled_for: None,
                output_dir: None,
                priority: 2,
                max_retries: 0,
            },
        )
        .await
        .expect("enqueue");
        sqlx::query("UPDATE execution_queue SET status = 'RUNNING' WHERE id = ?")
            .bind(&job.id)
            .execute(pool)
            .await
            .expect("running");
        job.id
    }

    #[tokio::test]
    async fn coalesces_repeated_heartbeats_into_one_write() {
        let ctx = crate::test_support::context().await;
        let buffer = WriteBuffer::start(ctx.db.clone(), Duration::from_secs(30), 64);
        let job_id = running_job(&ctx.db, &ctx.workspace_id, 0).await;

        for _ in 0..50 {
            assert!(buffer.job_heartbeat(&job_id).await);
        }
        buffer.shutdown().await;

        let metrics = buffer.metrics();
        assert_eq!(metrics.queued, 50);
        assert!(metrics.coalesced >= 49, "metrics: {metrics:?}");
        assert_eq!(metrics.failed, 0);
        assert_eq!(metrics.pending, 0);

        let heartbeat: Option<String> =
            sqlx::query_scalar("SELECT last_heartbeat_at FROM execution_queue WHERE id = ?")
                .bind(&job_id)
                .fetch_one(&ctx.db)
                .await
                .expect("heartbeat");
        assert!(heartbeat.is_some());
    }

    #[tokio::test]
    async fn concurrent_run_and_node_heartbeats_never_lock_the_database() {
        let ctx = crate::test_support::context().await;
        let buffer = WriteBuffer::start(ctx.db.clone(), Duration::from_millis(10), 32);

        let mut jobs = Vec::new();
        for index in 0..4 {
            jobs.push(running_job(&ctx.db, &ctx.workspace_id, index).await);
        }
        let mut nodes = Vec::new();
        for index in 0..8 {
            let id = format!("worker-{index}");
            sqlx::query("INSERT INTO workers (id, name, endpoint) VALUES (?, ?, ?)")
                .bind(&id)
                .bind(&id)
                .bind("http://127.0.0.1:4096")
                .execute(&ctx.db)
                .await
                .expect("worker");
            nodes.push(id);
        }

        let mut tasks = Vec::new();
        for job_id in jobs.clone() {
            let buffer = buffer.clone();
            tasks.push(tokio::spawn(async move {
                for _ in 0..100 {
                    assert!(buffer.job_heartbeat(&job_id).await);
                }
            }));
        }
        for node_id in nodes.clone() {
            let buffer = buffer.clone();
            tasks.push(tokio::spawn(async move {
                for _ in 0..100 {
                    assert!(buffer.node_seen(&node_id).await);
                }
            }));
        }
        for task in tasks {
            task.await.expect("producer");
        }
        buffer.shutdown().await;

        let metrics = buffer.metrics();
        assert_eq!(metrics.failed, 0, "metrics: {metrics:?}");
        assert_eq!(metrics.dropped, 0);
        assert_eq!(metrics.pending, 0);
        assert_eq!(metrics.queued, 4 * 100 + 8 * 100);
        assert!(metrics.coalesced > 0, "metrics: {metrics:?}");

        let seen: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM workers WHERE last_seen_at IS NOT NULL")
                .fetch_one(&ctx.db)
                .await
                .expect("seen");
        assert_eq!(seen, 8);
        let beats: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM execution_queue WHERE last_heartbeat_at IS NOT NULL",
        )
        .fetch_one(&ctx.db)
        .await
        .expect("beats");
        assert_eq!(beats, 4);
    }
}
