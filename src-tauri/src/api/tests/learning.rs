use super::*;
use crate::db::runs::{CreateRunInput, RunUsage};

async fn seed_failed_run(ctx: &TestContext, id: &str, message: &str) {
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
        "FAILED",
        &RunUsage::default(),
        900,
        None,
        None,
    )
    .await
    .expect("finish");
    runs::set_error(&ctx.state.db, id, "timeout", message, "hint")
        .await
        .expect("error");
}

#[tokio::test]
async fn learning_loop_scans_promotes_and_applies_after_approval() {
    let ctx = test_support::context().await;
    let workspace = format!("?workspace_id={}", ctx.workspace_id);

    let (status, overview) = call(
        &ctx,
        Method::GET,
        &format!("/api/learning{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(overview["lessons"].as_array().expect("lessons").len(), 0);
    assert_eq!(overview["settings"]["enabled"], true);

    seed_failed_run(&ctx, "run-learning", "timed out after 60s").await;
    let (status, scan) = call(
        &ctx,
        Method::POST,
        &format!("/api/learning/scan{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(scan["analyzed"], 1);
    assert_eq!(scan["lessons"], 1);

    let (status, overview) = call(
        &ctx,
        Method::GET,
        &format!("/api/learning{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let lesson_id = overview["lessons"][0]["id"].as_str().expect("lesson id");
    assert_eq!(overview["lessons"][0]["category"], "failure_pattern");
    assert_eq!(overview["lessons"][0]["occurrences"], 1);

    // A manual scan is idempotent and does not inflate occurrences.
    let (_, again) = call(
        &ctx,
        Method::POST,
        &format!("/api/learning/scan{workspace}"),
        None,
    )
    .await;
    assert_eq!(again["lessons"], 0);

    let (status, remembered) = call(
        &ctx,
        Method::POST,
        &format!("/api/learning/lessons/{lesson_id}/remember{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(remembered["remembered_at"].is_string());
    assert!(ctx.workspace_root.join(".opencode/MEMORY.md").exists());

    let (status, forgotten) = call(
        &ctx,
        Method::POST,
        &format!("/api/learning/lessons/{lesson_id}/forget{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(forgotten["remembered_at"].is_null());
    assert!(
        !std::fs::read_to_string(ctx.workspace_root.join(".opencode/MEMORY.md"))
            .expect("memory")
            .contains(&format!(
                "<!-- learning:{} -->",
                forgotten["fingerprint"].as_str().unwrap()
            ))
    );

    let (status, dismissed) = call(
        &ctx,
        Method::POST,
        &format!("/api/learning/lessons/{lesson_id}/dismiss{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(dismissed["status"], "DISMISSED");
    let (status, restored) = call(
        &ctx,
        Method::POST,
        &format!("/api/learning/lessons/{lesson_id}/restore{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(restored["status"], "OPEN");

    let (status, proposal) = call(
        &ctx,
        Method::POST,
        &format!("/api/learning/lessons/{lesson_id}/promote{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(proposal["kind"], "SKILL");
    assert_eq!(proposal["status"], "PENDING");
    let proposal_id = proposal["id"].as_str().expect("proposal id");
    let payload: Value = serde_json::from_str(proposal["payload_json"].as_str().unwrap()).unwrap();
    let skill_id = payload["id"].as_str().expect("skill id");

    let (status, pending) = call(
        &ctx,
        Method::GET,
        &format!("/api/proposals{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(pending["proposals"][0]["id"], proposal_id);

    let skill_path = ctx
        .workspace_root
        .join(".opencode/skills")
        .join(skill_id)
        .join("SKILL.md");
    assert!(!skill_path.exists());

    let (status, resolved) = call(
        &ctx,
        Method::POST,
        &format!("/api/proposals/{proposal_id}/resolve"),
        Some(json!({ "approved": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "resolve: {resolved}");
    assert_eq!(resolved["status"], "APPLIED");
    assert!(
        skill_path.exists(),
        "skill should be written after approval"
    );

    let (_, overview) = call(
        &ctx,
        Method::GET,
        &format!("/api/learning{workspace}"),
        None,
    )
    .await;
    assert_eq!(overview["lessons"][0]["status"], "PROMOTED");
    assert_eq!(overview["lessons"][0]["proposal_id"], proposal_id);
    assert_eq!(overview["proposals"].as_array().expect("pending").len(), 0);
}

#[tokio::test]
async fn learning_settings_can_disable_capture() {
    let ctx = test_support::context().await;
    let workspace = format!("?workspace_id={}", ctx.workspace_id);
    let (status, saved) = call(
        &ctx,
        Method::PUT,
        &format!("/api/learning/settings{workspace}"),
        Some(json!({
            "enabled": false,
            "capture_successes": false,
            "auto_propose": false,
            "min_occurrences": 3
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["enabled"], false);
    assert_eq!(saved["min_occurrences"], 3);

    seed_failed_run(&ctx, "run-disabled", "timed out").await;
    let (_, scan) = call(
        &ctx,
        Method::POST,
        &format!("/api/learning/scan{workspace}"),
        None,
    )
    .await;
    assert_eq!(scan["lessons"], 0);
}

#[tokio::test]
async fn manual_learning_is_saved_and_can_be_remembered() {
    let ctx = test_support::context().await;
    let (status, lesson) = call(
        &ctx,
        Method::POST,
        "/api/learning/lessons",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "title": "Preferência de resposta",
            "detail": "Responder em português e citar fontes.",
            "category": "manual",
            "remember": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{lesson}");
    assert_eq!(lesson["category"], "manual");
    assert!(lesson["remembered_at"].is_string());
    let memory =
        std::fs::read_to_string(ctx.workspace_root.join(".opencode/MEMORY.md")).expect("memory");
    assert!(memory.contains("Preferência de resposta"));
    assert!(memory.contains("Responder em português"));
}
