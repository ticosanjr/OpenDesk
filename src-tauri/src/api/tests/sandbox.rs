use super::*;

#[tokio::test]
async fn sandbox_policy_roundtrips() {
    let ctx = test_support::context().await;
    let (status, current) = call(&ctx, Method::GET, "/api/sandbox", None).await;
    assert_eq!(status, StatusCode::OK, "{current}");
    assert_eq!(current["policy"]["mode"], "off");

    let (status, updated) = call(
        &ctx,
        Method::PUT,
        "/api/sandbox",
        Some(json!({
            "mode": "strict",
            "network": false,
            "max_runtime_minutes": 10,
            "allowed_commands": ["claude"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["mode"], "strict");

    let (status, _) = call(
        &ctx,
        Method::PUT,
        "/api/sandbox",
        Some(json!({ "mode": "root" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn computer_status_and_session_lifecycle() {
    let ctx = test_support::context().await;
    let (status, capabilities) = call(&ctx, Method::GET, "/api/computer/status", None).await;
    assert_eq!(status, StatusCode::OK, "{capabilities}");
    assert!(capabilities["backend"].is_string());

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/computer/sessions",
        Some(json!({ "require_approval": true })),
    )
    .await;
    assert!(
        matches!(status, StatusCode::OK | StatusCode::CONFLICT),
        "{status}"
    );

    let session = crate::computer::create(&ctx.state.db, &ctx.workspace_id, None, true)
        .await
        .expect("session");
    let (status, approved) = call(
        &ctx,
        Method::POST,
        &format!("/api/computer/sessions/{}/approve", session.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{approved}");
    assert_eq!(approved["status"], "ACTIVE");

    let (status, ended) = call(
        &ctx,
        Method::POST,
        &format!("/api/computer/sessions/{}/end", session.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ended}");
    assert_eq!(ended["status"], "ENDED");
}
