use super::*;
use crate::queue_scheduler::job_dto;

#[test]
fn split_model_handles_provider_and_variant() {
    assert_eq!(
        split_model("opencode-go/glm-5.3#high"),
        (Some("opencode-go".into()), "glm-5.3".into())
    );
    assert_eq!(
        split_model("deepseek/deepseek-flash"),
        (Some("deepseek".into()), "deepseek-flash".into())
    );
    assert_eq!(split_model("plain-model"), (None, "plain-model".into()));
    assert_eq!(
        split_model("provider/model#"),
        (Some("provider".into()), "model".into())
    );
}

#[test]
fn job_dto_preserves_operational_fields() {
    let dto = job_dto(
        queue::QueueJob {
            id: "job-1".into(),
            workspace_id: "ws-1".into(),
            task_id: Some("task-1".into()),
            schedule_id: None,
            session_id: Some("session-1".into()),
            concurrency_group: Some("web_scraper".into()),
            prompt_payload: "prompt".into(),
            resolved_mode: "build".into(),
            resolved_agent: "build".into(),
            resolved_model: "opencode-go/glm-5.3".into(),
            resolved_effort: "high".into(),
            resolved_harness: "opencode".into(),
            assigned_node_id: None,
            job_kind: "task".into(),
            scheduled_for: None,
            output_dir: Some("/tmp/out".into()),
            priority: 3,
            status: "QUEUED".into(),
            retry_count: 1,
            max_retries: 2,
            error_message: Some("retry me".into()),
            enqueued_at: "2026-11-11T00:00:00Z".into(),
            started_at: None,
            finished_at: None,
            lease_expires_at: Some("2026-11-11T00:01:00Z".into()),
            last_heartbeat_at: Some("2026-11-11T00:00:30Z".into()),
            task_title: Some("Relatório".into()),
            task_origin: Some("MANUAL".into()),
        },
        Some("Aguardando slot livre".into()),
    );
    assert_eq!(dto.id, "job-1");
    assert_eq!(dto.wait_reason.as_deref(), Some("Aguardando slot livre"));
    assert_eq!(dto.title.as_deref(), Some("Relatório"));
    assert_eq!(dto.priority, 3);
    assert_eq!(dto.concurrency_group.as_deref(), Some("web_scraper"));
    assert_eq!(dto.retry_count, 1);
    assert_eq!(dto.error_message.as_deref(), Some("retry me"));
}

#[tokio::test]
async fn worktree_runs_setting_defaults_off_and_can_be_enabled() {
    let ctx = crate::test_support::context().await;
    assert!(!worktree_runs_enabled(&ctx.db).await);
    opendesk_storage::repos::settings::set(&ctx.db, "worktree_runs", &serde_json::json!(true))
        .await
        .expect("setting");
    assert!(worktree_runs_enabled(&ctx.db).await);
}

#[test]
fn ab_tasks_force_worktree_even_when_setting_is_off() {
    assert!(!forces_worktree(Some("[]")));
    assert!(forces_worktree(Some(r#"["ab:group-1","ab-slot:a"]"#)));
    assert!(!forces_worktree(Some("not-json")));
    assert!(!forces_worktree(None));
}

#[test]
fn concurrency_is_clamped_to_a_safe_range() {
    assert_eq!(clamp_concurrency(0), 1);
    assert_eq!(clamp_concurrency(1), 1);
    assert_eq!(clamp_concurrency(3), 3);
    assert_eq!(clamp_concurrency(32), 32);
    assert_eq!(clamp_concurrency(99), 32);
}
