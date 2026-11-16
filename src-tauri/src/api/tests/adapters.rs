use super::*;

#[tokio::test]
async fn lists_builtin_harnesses_with_a_default() {
    let ctx = test_support::context().await;
    let (status, body) = call(&ctx, Method::GET, "/api/adapters", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let list = body.as_array().expect("array");
    assert_eq!(list.len(), 5);
    let default = list
        .iter()
        .find(|item| item["is_default"] == true)
        .expect("default harness");
    assert_eq!(default["id"], "opencode");
}

#[tokio::test]
async fn updates_command_and_reports_health() {
    let ctx = test_support::context().await;
    let (status, updated) = call(
        &ctx,
        Method::PATCH,
        "/api/adapters/codex",
        Some(json!({ "command": "/bin/echo", "args": ["{prompt}"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["command"], "/bin/echo");

    let (status, health) = call(&ctx, Method::POST, "/api/adapters/codex/health", None).await;
    assert_eq!(status, StatusCode::OK, "{health}");
    assert_eq!(health["status"], "READY");
}

#[tokio::test]
async fn switches_the_workspace_default() {
    let ctx = test_support::context().await;
    let (status, body) = call(
        &ctx,
        Method::POST,
        "/api/adapters/default",
        Some(json!({ "harness": "claude-code" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["default_harness"], "claude-code");

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/adapters/default",
        Some(json!({ "harness": "nope" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
