#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
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
        let deadline = next_lease_deadline(&pool)
            .await
            .expect("deadline")
            .expect("running lease has a deadline");
        assert!(!deadline.is_empty());
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
        assert!(next_lease_deadline(&pool)
            .await
            .expect("deadline")
            .is_some());
        finish(&pool, &job.id, "FAILED", Some("expired"))
            .await
            .expect("finish");
        assert!(next_lease_deadline(&pool).await.expect("deadline").is_none());
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

#[tokio::test]
async fn queue_policy_roundtrip_normalizes_limits() {
    let (_temp, pool) = setup().await;
    let policy = QueuePolicy {
        max_concurrency: 40,
        max_queue_size: 0,
        default_agent_concurrency: 0,
        group_limits: HashMap::from([("default".into(), 99)]),
    };
    let saved = save_policy(&pool, &policy).await.expect("save");
    assert_eq!(saved.max_concurrency, 32);
    assert_eq!(saved.max_queue_size, 1);
    assert_eq!(saved.default_agent_concurrency, 1);
    assert_eq!(saved.group_limits["default"], 32);
    assert_eq!(load_policy(&pool).await.expect("load"), saved);
}

#[tokio::test]
async fn queue_full_rejects_new_jobs_until_a_slot_frees() {
    let (_temp, pool) = setup().await;
    save_policy(
        &pool,
        &QueuePolicy {
            max_concurrency: 2,
            max_queue_size: 2,
            default_agent_concurrency: 2,
            group_limits: HashMap::from([("default".into(), 2)]),
        },
    )
    .await
    .expect("policy");
    let mut first = input();
    first.prompt_payload = "first".into();
    let first = enqueue(&pool, first).await.expect("first");
    let mut second = input();
    second.prompt_payload = "second".into();
    let second = enqueue(&pool, second).await.expect("second");

    let mut third = input();
    third.prompt_payload = "third".into();
    let error = enqueue(&pool, third).await.expect_err("queue full");
    assert!(matches!(error, AppError::QueueFull(_)));

    finish(&pool, &first.id, "COMPLETED", None)
        .await
        .expect("finish");
    let mut fourth = input();
    fourth.prompt_payload = "fourth".into();
    enqueue(&pool, fourth).await.expect("after slot frees");
    assert_eq!(get(&pool, &second.id).await.expect("second").status, "QUEUED");
}

#[tokio::test]
async fn parallel_claims_fill_three_slots() {
    let (_temp, pool) = setup().await;
    let policy = QueuePolicy {
        max_concurrency: 3,
        max_queue_size: 20,
        default_agent_concurrency: 3,
        group_limits: HashMap::from([("default".into(), 3)]),
    };
    save_policy(&pool, &policy).await.expect("policy");
    let mut expected = Vec::new();
    for index in 0..3 {
        let mut job = input();
        job.prompt_payload = format!("parallel-{index}");
        job.session_id = Some(format!("session-{index}"));
        expected.push(enqueue(&pool, job).await.expect("enqueue").id);
    }
    let barrier = Arc::new(tokio::sync::Barrier::new(3));
    let mut handles = Vec::new();
    for _ in 0..3 {
        let pool = pool.clone();
        let policy = policy.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            barrier.wait().await;
            for _ in 0..50 {
                if let Some(job) = next_eligible(&pool, &policy).await.expect("eligible")
                    && claim(&pool, &job.id).await.expect("claim")
                {
                    return job.id;
                }
                tokio::task::yield_now().await;
            }
            panic!("no eligible job");
        }));
    }
    let mut claimed = Vec::new();
    for handle in handles {
        claimed.push(handle.await.expect("join"));
    }
    claimed.sort();
    expected.sort();
    assert_eq!(claimed, expected);
    let running: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM execution_queue WHERE status = 'RUNNING'")
            .fetch_one(&pool)
            .await
            .expect("running");
    assert_eq!(running, 3);
}

#[tokio::test]
async fn head_of_line_blocking_skips_a_full_group() {
    let (_temp, pool) = setup().await;
    let policy = QueuePolicy {
        max_concurrency: 4,
        max_queue_size: 20,
        default_agent_concurrency: 4,
        group_limits: HashMap::from([
            ("local_llm_heavy".into(), 1),
            ("light_api".into(), 5),
            ("default".into(), 4),
        ]),
    };
    save_policy(&pool, &policy).await.expect("policy");
    let mut heavy = input();
    heavy.concurrency_group = Some("local_llm_heavy".into());
    heavy.prompt_payload = "heavy-1".into();
    heavy.priority = 4;
    heavy.resolved_effort = "high".into();
    let first = enqueue(&pool, heavy.clone()).await.expect("heavy 1");
    heavy.prompt_payload = "heavy-2".into();
    heavy.resolved_effort = "low".into();
    let blocked = enqueue(&pool, heavy).await.expect("heavy 2");
    let mut light = input();
    light.concurrency_group = Some("light_api".into());
    light.prompt_payload = "light-1".into();
    light.resolved_agent = "scout".into();
    light.priority = 1;
    let light = enqueue(&pool, light).await.expect("light");

    assert!(claim(&pool, &first.id).await.expect("claim heavy"));
    let next = next_eligible(&pool, &policy)
        .await
        .expect("next")
        .expect("light job");
    assert_eq!(next.id, light.id);
    assert_ne!(next.id, blocked.id);
}

#[tokio::test]
async fn same_task_or_session_is_not_claimed_twice() {
    let (_temp, pool) = setup().await;
    let policy = QueuePolicy {
        max_concurrency: 4,
        max_queue_size: 20,
        default_agent_concurrency: 4,
        group_limits: HashMap::from([("default".into(), 4)]),
    };
    save_policy(&pool, &policy).await.expect("policy");
    sqlx::query(
        "INSERT INTO tasks (id, workspace_id, title, description)
         VALUES ('task-1', 'ws-1', 'Task', 'Description')",
    )
    .execute(&pool)
    .await
    .expect("task");
    let mut first = input();
    first.task_id = Some("task-1".into());
    first.prompt_payload = "same-task-1".into();
    let first = enqueue(&pool, first).await.expect("first");
    let mut second = input();
    second.task_id = Some("task-1".into());
    second.session_id = Some("session-2".into());
    second.prompt_payload = "same-task-2".into();
    let second = enqueue(&pool, second).await.expect("second");
    assert!(claim(&pool, &first.id).await.expect("claim"));
    assert!(next_eligible(&pool, &policy)
        .await
        .expect("next")
        .is_none());
    finish(&pool, &first.id, "COMPLETED", None)
        .await
        .expect("finish");
    assert_eq!(
        next_eligible(&pool, &policy)
            .await
            .expect("next")
            .expect("second")
            .id,
        second.id
    );
}

#[tokio::test]
async fn default_agent_quota_limits_parallelism() {
    let (_temp, pool) = setup().await;
    let policy = QueuePolicy {
        max_concurrency: 4,
        max_queue_size: 20,
        default_agent_concurrency: 1,
        group_limits: HashMap::from([("default".into(), 4)]),
    };
    save_policy(&pool, &policy).await.expect("policy");
    let mut first = input();
    first.prompt_payload = "agent-1".into();
    let first = enqueue(&pool, first).await.expect("first");
    let mut second = input();
    second.prompt_payload = "agent-2".into();
    let second = enqueue(&pool, second).await.expect("second");
    assert!(claim(&pool, &first.id).await.expect("claim"));
    assert!(next_eligible(&pool, &policy)
        .await
        .expect("next")
        .is_none());
    finish(&pool, &first.id, "COMPLETED", None)
        .await
        .expect("finish");
    assert_eq!(
        next_eligible(&pool, &policy)
            .await
            .expect("next")
            .expect("second")
            .id,
        second.id
    );
}

#[tokio::test]
async fn wait_reasons_explain_group_pause_and_schedule() {
    let (_temp, pool) = setup().await;
    let policy = QueuePolicy {
        max_concurrency: 2,
        max_queue_size: 20,
        default_agent_concurrency: 2,
        group_limits: HashMap::from([("default".into(), 1)]),
    };
    save_policy(&pool, &policy).await.expect("policy");
    let running = enqueue(&pool, input()).await.expect("running");
    assert!(claim(&pool, &running.id).await.expect("claim"));
    let mut waiting = input();
    waiting.prompt_payload = "waiting".into();
    let waiting = enqueue(&pool, waiting).await.expect("waiting");
    let reasons = wait_reasons(&pool, std::slice::from_ref(&waiting), &policy, false)
        .await
        .expect("reasons");
    assert_eq!(
        reasons.get(&waiting.id).map(String::as_str),
        Some("Grupo default cheio (1/1)")
    );
    let paused = wait_reasons(&pool, std::slice::from_ref(&waiting), &policy, true)
        .await
        .expect("paused");
    assert_eq!(paused.get(&waiting.id).map(String::as_str), Some("Fila pausada"));
    let mut scheduled = input();
    scheduled.prompt_payload = "scheduled".into();
    scheduled.scheduled_for = Some("2999-01-01T00:00:00.000Z".into());
    let scheduled = enqueue(&pool, scheduled).await.expect("scheduled");
    let reasons = wait_reasons(&pool, std::slice::from_ref(&scheduled), &policy, false)
        .await
        .expect("scheduled reasons");
    assert_eq!(
        reasons.get(&scheduled.id).map(String::as_str),
        Some("Aguardando horário agendado")
    );
}
}
