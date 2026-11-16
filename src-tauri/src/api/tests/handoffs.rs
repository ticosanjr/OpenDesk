use super::*;

async fn create_handoff(ctx: &TestContext, target: &str, budget: Option<f64>) -> Value {
    let (status, handoff) = call(
        ctx,
        Method::POST,
        "/api/handoffs",
        Some(json!({
            "source_agent": "build",
            "target_agent": target,
            "objective": "Revisar o relatório final",
            "input_artifacts": ["output/report.md"],
            "acceptance": ["sem erros de português"],
            "sub_budget_usd": budget
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{handoff}");
    handoff
}

#[tokio::test]
async fn accepts_and_creates_the_target_task() {
    let ctx = test_support::context().await;
    let handoff = create_handoff(&ctx, "review", None).await;
    assert_eq!(handoff["status"], "PENDING");

    let (status, body) = call(
        &ctx,
        Method::POST,
        &format!(
            "/api/handoffs/{}/accept",
            handoff["id"].as_str().expect("id")
        ),
        Some(json!({ "note": "pode começar" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["handoff"]["status"], "ACCEPTED");
    assert_eq!(body["task"]["assigned_agent"], "review");
    assert_eq!(body["task"]["origin"], "DELEGATION");
    assert!(
        body["task"]["description"]
            .as_str()
            .is_some_and(|text| text.contains("Critérios de aceite"))
    );
}

#[tokio::test]
async fn rejects_and_requires_pending_state() {
    let ctx = test_support::context().await;
    let handoff = create_handoff(&ctx, "review", None).await;
    let id = handoff["id"].as_str().expect("id");

    let (status, rejected) = call(
        &ctx,
        Method::POST,
        &format!("/api/handoffs/{id}/reject"),
        Some(json!({ "reason": "fora de escopo" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rejected}");
    assert_eq!(rejected["status"], "REJECTED");

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/handoffs/{id}/accept"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn budget_guard_requires_force() {
    let ctx = test_support::context().await;
    call(
        &ctx,
        Method::PUT,
        "/api/budget",
        Some(json!({ "daily_limit_usd": 1.0, "hard_stop_enabled": true })),
    )
    .await;
    let handoff = create_handoff(&ctx, "review", Some(1000.0)).await;
    let id = handoff["id"].as_str().expect("id");

    let (status, body) = call(
        &ctx,
        Method::POST,
        &format!("/api/handoffs/{id}/accept"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    let (status, body) = call(
        &ctx,
        Method::POST,
        &format!("/api/handoffs/{id}/accept"),
        Some(json!({ "force": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn finish_requires_an_accepted_handoff() {
    let ctx = test_support::context().await;
    let handoff = create_handoff(&ctx, "review", None).await;
    let id = handoff["id"].as_str().expect("id");

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/handoffs/{id}/finish"),
        Some(json!({ "status": "DONE" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    call(
        &ctx,
        Method::POST,
        &format!("/api/handoffs/{id}/accept"),
        Some(json!({})),
    )
    .await;
    let (status, finished) = call(
        &ctx,
        Method::POST,
        &format!("/api/handoffs/{id}/finish"),
        Some(json!({ "status": "DONE", "result": { "ok": true } })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{finished}");
    assert_eq!(finished["status"], "DONE");
}
