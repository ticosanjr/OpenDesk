use super::*;

fn job() -> queue::QueueJob {
    queue::QueueJob {
        id: "job".into(),
        workspace_id: "ws".into(),
        task_id: None,
        schedule_id: None,
        session_id: None,
        concurrency_group: None,
        prompt_payload: String::new(),
        resolved_mode: "BUILD_DIRECT".into(),
        resolved_agent: "build".into(),
        resolved_model: "provider/model".into(),
        resolved_effort: "low".into(),
        resolved_harness: "opencode".into(),
        assigned_node_id: None,
        job_kind: "prompt".into(),
        scheduled_for: None,
        output_dir: None,
        priority: 1,
        max_retries: 0,
        retry_count: 0,
        status: "QUEUED".into(),
        task_title: Some("Tarefa".into()),
        enqueued_at: String::new(),
        started_at: None,
        finished_at: None,
        lease_expires_at: None,
        last_heartbeat_at: None,
        error_message: None,
        task_origin: Some("MANUAL".into()),
    }
}

#[test]
fn plan_uses_the_job_mode_and_slugged_title() {
    let temp = tempfile::TempDir::new().expect("temp");
    let paths = AppPaths {
        data_dir: temp.path().join("data"),
        config_dir: temp.path().join("config"),
        cache_dir: temp.path().join("cache"),
        log_dir: temp.path().join("logs"),
        db_path: temp.path().join("data/opendesk.db"),
        backups_dir: temp.path().join("backups"),
        artifacts_dir: temp.path().join("artifacts"),
        run_logs_dir: temp.path().join("runs"),
        daemon_lock_path: temp.path().join("daemon.json"),
        api_lock_path: temp.path().join("api.json"),
        allow_temp_workspaces: true,
    };
    let plan = plan_run(&job(), None, &paths, temp.path());
    assert_eq!(plan.phase, "BUILD_DIRECT");
    assert!(plan.log_path.ends_with(format!("{}.jsonl", plan.run_id)));
    assert!(plan.output_dir.ends_with("tarefa"));
}

#[tokio::test]
async fn records_diffs_and_writes_once() {
    use crate::opencode_client::rest_client::FileDiff;
    use crate::opencode_client::session_runner::{FileWrite, RunOutcome, RunStatus};

    let context = crate::test_support::context().await;
    opendesk_storage::repos::runs::create(
        &context.db,
        opendesk_storage::repos::runs::CreateRunInput {
            id: "run-artifacts".into(),
            queue_job_id: None,
            workspace_id: context.workspace_id.clone(),
            task_id: None,
            schedule_id: None,
            parent_run_id: None,
            phase: Some("build".into()),
            agent_name: "build".into(),
            execution_mode: "BUILD_DIRECT".into(),
            provider_id: Some("opencode-go".into()),
            model_id: "glm-5.3".into(),
            effort_level: "low".into(),
            pricing_snapshot_json: None,
            log_path: None,
        },
    )
    .await
    .expect("run");
    std::fs::write(context.workspace_root.join("a.md"), "a").expect("a");
    std::fs::write(context.workspace_root.join("b.txt"), "b").expect("b");
    let mut job = job();
    job.workspace_id = context.workspace_id.clone();
    let outcome = RunOutcome {
        status: RunStatus::Success,
        session_id: "session".into(),
        usage: opendesk_storage::repos::runs::RunUsage::default(),
        summary: None,
        diffs: vec![FileDiff {
            file: "a.md".into(),
            patch: "+a".into(),
            additions: 1,
            deletions: 0,
            status: "added".into(),
        }],
        writes: vec![
            FileWrite {
                path: "b.txt".into(),
                action: "MODIFIED".into(),
                diff_patch: None,
            },
            FileWrite {
                path: "a.md".into(),
                action: "MODIFIED".into(),
                diff_patch: None,
            },
        ],
        error: None,
        log_tail: None,
        permissions: Vec::new(),
    };
    record_artifacts(
        &context.db,
        &context.events,
        &job,
        "run-artifacts",
        "teste",
        &context.workspace_root,
        &context.workspace_root.join("no-output"),
        &outcome,
    )
    .await;
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM artifacts_provenance WHERE run_id = 'run-artifacts'",
    )
    .fetch_one(&context.db)
    .await
    .expect("count");
    assert_eq!(count, 2);
}

#[tokio::test]
async fn scans_shell_created_files_in_the_output_directory() {
    use crate::opencode_client::session_runner::{RunOutcome, RunStatus};

    let context = crate::test_support::context().await;
    let output = context.workspace_root.join("output").join("tarefa");
    std::fs::create_dir_all(&output).expect("output");
    std::fs::write(output.join("poesia.txt"), "v1").expect("file");
    let mut job = job();
    job.workspace_id = context.workspace_id.clone();

    let empty = || RunOutcome {
        status: RunStatus::Success,
        session_id: "session".into(),
        usage: opendesk_storage::repos::runs::RunUsage::default(),
        summary: None,
        diffs: Vec::new(),
        writes: Vec::new(),
        error: None,
        log_tail: None,
        permissions: Vec::new(),
    };
    let create_run = |id: &'static str| {
        let db = context.db.clone();
        let workspace_id = context.workspace_id.clone();
        async move {
            opendesk_storage::repos::runs::create(
                &db,
                opendesk_storage::repos::runs::CreateRunInput {
                    id: id.into(),
                    queue_job_id: None,
                    workspace_id,
                    task_id: None,
                    schedule_id: None,
                    parent_run_id: None,
                    phase: Some("build".into()),
                    agent_name: "build".into(),
                    execution_mode: "BUILD_DIRECT".into(),
                    provider_id: Some("opencode-go".into()),
                    model_id: "glm-5.3".into(),
                    effort_level: "low".into(),
                    pricing_snapshot_json: None,
                    log_path: None,
                },
            )
            .await
            .expect("run");
        }
    };

    create_run("run-scan-1").await;
    record_artifacts(
        &context.db,
        &context.events,
        &job,
        "run-scan-1",
        "teste",
        &context.workspace_root,
        &output,
        &empty(),
    )
    .await;
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM artifacts_provenance WHERE run_id = 'run-scan-1'")
            .fetch_one(&context.db)
            .await
            .expect("count");
    assert_eq!(count, 1);
    let artifact = opendesk_storage::repos::artifacts::latest_for_path(
        &context.db,
        &context.workspace_id,
        "output/tarefa/poesia.txt",
    )
    .await
    .expect("artifact")
    .expect("artifact row");
    assert_eq!(artifact.action_type, "CREATED");

    // Re-running the same run must not duplicate rows.
    record_artifacts(
        &context.db,
        &context.events,
        &job,
        "run-scan-1",
        "teste",
        &context.workspace_root,
        &output,
        &empty(),
    )
    .await;
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM artifacts_provenance WHERE run_id = 'run-scan-1'")
            .fetch_one(&context.db)
            .await
            .expect("count");
    assert_eq!(count, 1);

    create_run("run-scan-2").await;
    std::fs::write(output.join("poesia.txt"), "v2").expect("file");
    record_artifacts(
        &context.db,
        &context.events,
        &job,
        "run-scan-2",
        "teste",
        &context.workspace_root,
        &output,
        &empty(),
    )
    .await;
    let artifact = opendesk_storage::repos::artifacts::latest_for_path(
        &context.db,
        &context.workspace_id,
        "output/tarefa/poesia.txt",
    )
    .await
    .expect("artifact")
    .expect("artifact row");
    assert_eq!(artifact.run_id, "run-scan-2");
    assert_eq!(artifact.action_type, "MODIFIED");
    assert!(artifact.previous_sha256.is_some());
}

#[tokio::test]
async fn chat_artifacts_are_linked_to_a_chat_run() {
    use crate::opencode_client::rest_client::FileDiff;

    let context = crate::test_support::context().await;
    std::fs::write(context.workspace_root.join("chat.md"), "oi").expect("file");
    let run = opendesk_storage::repos::runs::create_chat(
        &context.db,
        opendesk_storage::repos::runs::CreateChatRunInput {
            id: "run-chat-1".into(),
            workspace_id: context.workspace_id.clone(),
            session_id: Some("ses-chat".into()),
            agent_name: "build".into(),
            provider_id: Some("opencode-go".into()),
            model_id: "deepseek".into(),
            channel: "telegram".into(),
            origin_ref: "thread-1".into(),
            started_at: chrono::Utc::now().to_rfc3339(),
        },
    )
    .await
    .expect("chat run");
    record_chat_artifacts(
        &context.db,
        &context.events,
        &context.workspace_id,
        &run.id,
        "build",
        "Chat · telegram",
        &context.workspace_root,
        &[FileDiff {
            file: "chat.md".into(),
            patch: "+oi".into(),
            additions: 1,
            deletions: 0,
            status: "added".into(),
        }],
        &[],
    )
    .await;
    let artifact = opendesk_storage::repos::artifacts::latest_for_path(
        &context.db,
        &context.workspace_id,
        "chat.md",
    )
    .await
    .expect("artifact")
    .expect("artifact row");
    assert_eq!(artifact.source, "chat");
    assert_eq!(artifact.channel.as_deref(), Some("telegram"));
    assert_eq!(artifact.origin_ref.as_deref(), Some("thread-1"));
}
