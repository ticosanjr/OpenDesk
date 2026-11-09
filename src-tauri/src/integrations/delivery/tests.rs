use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn retries_until_the_operation_succeeds() {
    let attempts = AtomicUsize::new(0);
    let result: anyhow::Result<&'static str> = with_retry(3, || {
        let attempt = attempts.fetch_add(1, Ordering::SeqCst) + 1;
        async move {
            if attempt < 3 {
                anyhow::bail!("transient")
            }
            Ok("ok")
        }
    })
    .await;
    assert_eq!(result.expect("third attempt"), "ok");
    assert_eq!(attempts.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn returns_the_last_error_after_all_attempts() {
    let attempts = AtomicUsize::new(0);
    let result: anyhow::Result<()> = with_retry(2, || {
        attempts.fetch_add(1, Ordering::SeqCst);
        async move { anyhow::bail!("still down") }
    })
    .await;
    assert!(result.is_err());
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

async fn due_delivery(
    channel: &str,
    key: &str,
    payload: serde_json::Value,
) -> (crate::test_support::TestContext, deliveries::Delivery) {
    let context = crate::test_support::context().await;
    let delivery =
        deliveries::create_pending(&context.state.db, &context.workspace_id, None, channel, key)
            .await
            .expect("create")
            .expect("created");
    deliveries::mark_failed_retryable(
        &context.state.db,
        &delivery.id,
        "transient",
        &payload.to_string(),
        0,
    )
    .await
    .expect("schedule");
    let due = deliveries::due_retries(&context.state.db, 10)
        .await
        .expect("due");
    let row = due
        .into_iter()
        .find(|row| row.id == delivery.id)
        .expect("due row");
    (context, row)
}

#[tokio::test]
async fn gdocs_retry_skips_when_the_artifact_is_gone() {
    let (context, delivery) = due_delivery(
        "GDOCS",
        "gdocs:retry:missing",
        serde_json::json!({ "artifact_id": "missing" }),
    )
    .await;
    retry_delivery(&context.state, &delivery)
        .await
        .expect("retry");
    let stored = deliveries::get_by_key(&context.state.db, "gdocs:retry:missing")
        .await
        .expect("get")
        .expect("row");
    assert_eq!(stored.status, "SKIPPED");
    assert_eq!(stored.last_error.as_deref(), Some("artifact not found"));
}

#[tokio::test]
async fn drive_retry_skips_when_the_artifact_is_gone() {
    let (context, delivery) = due_delivery(
        "DRIVE",
        "drive:retry:missing",
        serde_json::json!({ "artifact_id": "missing" }),
    )
    .await;
    retry_delivery(&context.state, &delivery)
        .await
        .expect("retry");
    let stored = deliveries::get_by_key(&context.state.db, "drive:retry:missing")
        .await
        .expect("get")
        .expect("row");
    assert_eq!(stored.status, "SKIPPED");
    assert_eq!(stored.last_error.as_deref(), Some("artifact not found"));
}

#[tokio::test]
async fn legacy_google_channels_are_skipped_instead_of_staying_failed() {
    let (context, delivery) = due_delivery(
        "GMAIL",
        "gmail:retry:legacy",
        serde_json::json!({ "to": "user@example.com" }),
    )
    .await;
    retry_delivery(&context.state, &delivery)
        .await
        .expect("retry");
    let stored = deliveries::get_by_key(&context.state.db, "gmail:retry:legacy")
        .await
        .expect("get")
        .expect("row");
    assert_eq!(stored.status, "SKIPPED");
    assert_eq!(
        stored.last_error.as_deref(),
        Some("delivery channel has no retry adapter")
    );
}

#[tokio::test]
async fn retry_rejects_invalid_payloads_without_touching_google() {
    let (context, delivery) = due_delivery(
        "DRIVE",
        "drive:retry:invalid",
        serde_json::json!({ "artifact_id": "unused" }),
    )
    .await;
    let mut broken = delivery.clone();
    broken.payload_json = Some("not-json".into());
    retry_delivery(&context.state, &broken)
        .await
        .expect("retry");
    let stored = deliveries::get_by_key(&context.state.db, "drive:retry:invalid")
        .await
        .expect("get")
        .expect("row");
    assert_eq!(stored.status, "SKIPPED");
    assert_eq!(stored.last_error.as_deref(), Some("invalid retry payload"));
}

fn test_run() -> runs::TaskRun {
    runs::TaskRun {
        id: "run-1".into(),
        queue_job_id: None,
        workspace_id: "ws-1".into(),
        task_id: None,
        schedule_id: None,
        attempt_number: 1,
        session_id: None,
        parent_run_id: None,
        phase: Some("build".into()),
        agent_name: "build".into(),
        execution_mode: "BUILD_DIRECT".into(),
        provider_id: Some("opencode-go".into()),
        model_id: "glm-5.3".into(),
        effort_level: "low".into(),
        status: "SUCCESS".into(),
        tokens_input: 0,
        tokens_output: 0,
        tokens_reasoning: 0,
        tokens_cache_read: 0,
        tokens_cache_write: 0,
        estimated_cost_usd: 0.25,
        pricing_snapshot_json: None,
        duration_ms: 1000,
        log_path: None,
        raw_stdout_log: None,
        summary_response: Some("<resumo> & pronto".into()),
        started_at: "2026-11-08T12:00:00Z".into(),
        completed_at: None,
        error_code: None,
        error_message: None,
        error_hint: None,
        task_title: Some("Tarefa".into()),
    }
}

fn test_artifact(path: &Path, index: usize) -> artifacts::ArtifactProvenance {
    artifacts::ArtifactProvenance {
        id: format!("artifact-{index}"),
        workspace_id: "ws-1".into(),
        task_id: None,
        run_id: "run-1".into(),
        file_path: format!("output/a{index}.md"),
        absolute_path: path.to_string_lossy().to_string(),
        file_type: "md".into(),
        file_size_bytes: 10,
        content_sha256: None,
        previous_sha256: None,
        action_type: "CREATED".into(),
        created_by_agent: "build".into(),
        creation_reason: "teste".into(),
        diff_patch: None,
        google_doc_url: None,
        google_drive_url: None,
        emailed_to: None,
        created_at: "2026-11-08T12:00:00Z".into(),
        pinned: false,
    }
}

#[test]
fn builds_escaped_html_and_formats_recipients() {
    let run = test_run();
    let html = build_html("<Tarefa>", &run, &[]);
    assert!(html.contains("<Tarefa>"));
    assert!(html.contains("&lt;resumo&gt; &amp; pronto"));
    assert!(html.contains("glm-5.3"));
    assert!(html.contains("0.2500"));
    assert_eq!(
        split_recipients(" a@b.com, c@d.com; ; e@f.com "),
        vec!["a@b.com", "c@d.com", "e@f.com"]
    );
}

#[test]
fn novelty_prefers_markdown_and_falls_back_to_summary() {
    let temp = tempfile::TempDir::new().expect("temp");
    let markdown = temp.path().join("a.md");
    let text = temp.path().join("b.txt");
    std::fs::write(&markdown, "markdown").expect("md");
    std::fs::write(&text, "texto").expect("txt");
    let mut plain = test_artifact(&text, 1);
    plain.file_path = "output/b.txt".into();
    let artifacts = vec![plain, test_artifact(&markdown, 2)];
    assert_eq!(
        novelty_hash(&artifacts, Some("resumo")),
        Some(hex_digest(b"markdown"))
    );
    assert_eq!(
        novelty_hash(&[], Some("resumo")),
        Some(hex_digest(b"resumo"))
    );
    assert!(novelty_hash(&[], None).is_none());
}

#[test]
fn attachments_respect_workspace_and_limit() {
    let temp = tempfile::TempDir::new().expect("temp");
    let root = temp.path();
    let mut artifacts = Vec::new();
    for index in 0..6 {
        let path = root.join(format!("a{index}.md"));
        std::fs::write(&path, b"conteudo").expect("file");
        artifacts.push(test_artifact(&path, index));
    }
    let outside = tempfile::NamedTempFile::new().expect("outside");
    artifacts.push(test_artifact(outside.path(), 99));
    let attachments = build_attachments(&artifacts, Some(root));
    assert_eq!(attachments.len(), 5);
    assert_eq!(attachments[0].mime, "text/markdown");
    assert_eq!(mime_for_path(Path::new("x.pdf")), "application/pdf");
    assert_eq!(
        mime_for_path(Path::new("x.unknown")),
        "application/octet-stream"
    );
}

#[test]
fn retry_backoff_is_bounded() {
    assert_eq!(retry_delay(0), 5);
    assert_eq!(retry_delay(1), 10);
    assert_eq!(retry_delay(10), 320);
}

#[tokio::test]
async fn deliver_run_without_recipients_is_a_noop() {
    let ctx = crate::test_support::context().await;
    runs::create(
        &ctx.state.db,
        runs::CreateRunInput {
            id: "run-noop".into(),
            queue_job_id: None,
            workspace_id: ctx.workspace_id.clone(),
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
    runs::finish(
        &ctx.state.db,
        "run-noop",
        "SUCCESS",
        &runs::RunUsage::default(),
        10,
        Some("ok"),
        None,
    )
    .await
    .expect("finish");
    deliver_run(
        &ctx.state.db,
        &ctx.state.paths,
        &ctx.workspace_id,
        None,
        None,
        "run-noop",
        ctx.state.google.clone(),
    )
    .await
    .expect("deliver");
}
