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
