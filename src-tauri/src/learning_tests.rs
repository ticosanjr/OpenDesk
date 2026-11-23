use super::*;
use crate::db::runs::{self, CreateRunInput, RunUsage};
use crate::test_support;

async fn seed_run(
    ctx: &test_support::TestContext,
    id: &str,
    status: &str,
    error: Option<(&str, &str, &str)>,
    summary: Option<&str>,
) {
    runs::create(
        &ctx.state.db,
        CreateRunInput {
            id: id.into(),
            queue_job_id: None,
            workspace_id: ctx.workspace_id.clone(),
            task_id: None,
            schedule_id: None,
            parent_run_id: None,
            phase: None,
            agent_name: "build".into(),
            execution_mode: "build".into(),
            provider_id: Some("opencode-go".into()),
            model_id: "glm-5.3".into(),
            effort_level: "high".into(),
            pricing_snapshot_json: None,
            log_path: None,
        },
    )
    .await
    .expect("run");
    runs::finish(
        &ctx.state.db,
        id,
        status,
        &RunUsage::default(),
        1200,
        summary,
        None,
    )
    .await
    .expect("finish");
    if let Some((code, message, hint)) = error {
        runs::set_error(&ctx.state.db, id, code, message, hint)
            .await
            .expect("error");
    }
}

#[tokio::test]
async fn capture_deduplicates_by_fingerprint_and_is_idempotent() {
    let ctx = test_support::context().await;
    seed_run(
        &ctx,
        "run-1",
        "FAILED",
        Some(("engine_offline", "OpenCode daemon is offline", "hint")),
        None,
    )
    .await;
    let first = capture_run(&ctx.state.db, "run-1")
        .await
        .expect("capture")
        .expect("lesson");
    assert_eq!(first.occurrences, 1);
    assert_eq!(first.category, "failure_pattern");

    // Re-capturing the same run is a no-op.
    assert!(
        capture_run(&ctx.state.db, "run-1")
            .await
            .expect("capture")
            .is_none()
    );

    seed_run(
        &ctx,
        "run-2",
        "FAILED",
        Some((
            "engine_offline",
            "OpenCode daemon is offline (attempt 2)",
            "hint",
        )),
        None,
    )
    .await;
    let second = capture_run(&ctx.state.db, "run-2")
        .await
        .expect("capture")
        .expect("lesson");
    assert_eq!(second.id, first.id);
    assert_eq!(second.occurrences, 2);
}

#[tokio::test]
async fn promote_creates_pending_skill_proposal() {
    let ctx = test_support::context().await;
    seed_run(
        &ctx,
        "run-3",
        "FAILED",
        Some(("timeout", "timed out after 60s", "hint")),
        None,
    )
    .await;
    let lesson = capture_run(&ctx.state.db, "run-3")
        .await
        .expect("capture")
        .expect("lesson");
    let proposal = promote(&ctx.state.db, &ctx.workspace_id, &lesson.id, Some("test"))
        .await
        .expect("promote");
    assert_eq!(proposal.kind, "SKILL");
    assert_eq!(proposal.status, "PENDING");
    assert_eq!(proposal.action, "CREATE");
    let payload: serde_json::Value = serde_json::from_str(&proposal.payload_json).unwrap();
    assert!(
        payload["id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("learned-failure-pattern-")
    );

    let promoted = get_lesson(&ctx.state.db, &ctx.workspace_id, &lesson.id)
        .await
        .expect("lesson");
    assert_eq!(promoted.status, "PROMOTED");
    assert_eq!(promoted.proposal_id.as_deref(), Some(proposal.id.as_str()));

    // Calling promote again returns the same proposal.
    let again = promote(&ctx.state.db, &ctx.workspace_id, &lesson.id, None)
        .await
        .expect("again");
    assert_eq!(again.id, proposal.id);
}

#[tokio::test]
async fn settings_and_dismiss_gate_the_loop() {
    let ctx = test_support::context().await;
    let settings = set_settings(
        &ctx.state.db,
        &ctx.workspace_id,
        LearningSettings {
            enabled: false,
            ..LearningSettings::default()
        },
    )
    .await
    .expect("settings");
    assert!(!settings.enabled);
    seed_run(
        &ctx,
        "run-4",
        "FAILED",
        Some(("timeout", "timeout", "hint")),
        None,
    )
    .await;
    assert!(
        capture_run(&ctx.state.db, "run-4")
            .await
            .expect("capture")
            .is_none()
    );

    set_settings(
        &ctx.state.db,
        &ctx.workspace_id,
        LearningSettings::default(),
    )
    .await
    .expect("settings");
    let lesson = capture_run(&ctx.state.db, "run-4")
        .await
        .expect("capture")
        .expect("lesson");
    let dismissed = dismiss(&ctx.state.db, &ctx.workspace_id, &lesson.id)
        .await
        .expect("dismiss");
    assert_eq!(dismissed.status, "DISMISSED");
    assert!(
        promote(&ctx.state.db, &ctx.workspace_id, &lesson.id, None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn remember_writes_memory_once() {
    let ctx = test_support::context().await;
    seed_run(
        &ctx,
        "run-memory",
        "FAILED",
        Some(("timeout", "timed out after 60s", "hint")),
        None,
    )
    .await;
    let lesson = capture_run(&ctx.state.db, "run-memory")
        .await
        .expect("capture")
        .expect("lesson");
    let first = remember(
        &ctx.state.db,
        &ctx.workspace_root,
        &ctx.state.paths.backups_dir,
        &ctx.workspace_id,
        &lesson.id,
    )
    .await
    .expect("remember");
    assert!(first.remembered_at.is_some());
    let path = ctx.workspace_root.join(".opencode/MEMORY.md");
    let content = std::fs::read_to_string(&path).expect("memory");
    assert!(
        content.contains("## Aprendizado: Falha recorrente: timeout"),
        "content: {content}"
    );
    assert!(content.contains(&format!("<!-- learning:{} -->", lesson.fingerprint)));

    remember(
        &ctx.state.db,
        &ctx.workspace_root,
        &ctx.state.paths.backups_dir,
        &ctx.workspace_id,
        &lesson.id,
    )
    .await
    .expect("remember again");
    let content = std::fs::read_to_string(&path).expect("memory");
    assert_eq!(
        content
            .matches(&format!("<!-- learning:{} -->", lesson.fingerprint))
            .count(),
        1,
        "memory block must be idempotent"
    );
}

#[tokio::test]
async fn stats_reports_recent_runs_and_lessons() {
    let ctx = test_support::context().await;
    seed_run(&ctx, "run-5", "SUCCESS", None, Some("ok")).await;
    seed_run(
        &ctx,
        "run-6",
        "FAILED",
        Some(("timeout", "timeout", "hint")),
        None,
    )
    .await;
    capture_run(&ctx.state.db, "run-6")
        .await
        .expect("capture")
        .expect("lesson");
    let stats = stats(&ctx.state.db, &ctx.workspace_id)
        .await
        .expect("stats");
    assert_eq!(stats.total, 1);
    assert_eq!(stats.open, 1);
    assert_eq!(stats.runs_analyzed, 2);
    assert_eq!(stats.failed_runs, 1);
    assert!((stats.success_rate - 0.5).abs() < f64::EPSILON);
}
