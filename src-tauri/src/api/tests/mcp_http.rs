use super::*;

#[tokio::test]
async fn http_mcp_gateway_initializes_and_lists_tools() {
    let ctx = test_support::context().await;
    let path = format!("/api/mcp?workspace_id={}", ctx.workspace_id);
    let (status, init) = call(
        &ctx,
        Method::POST,
        &path,
        Some(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{init}");
    assert_eq!(init["result"]["serverInfo"]["name"], "opendesk-control");

    let (status, list) = call(
        &ctx,
        Method::POST,
        &path,
        Some(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert!(
        list["result"]["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .any(|tool| tool["name"] == "task_list"),
        "{list}"
    );
}

#[tokio::test]
async fn role_read_hides_build_tools() {
    let ctx = test_support::context().await;
    let request = axum::http::Request::builder()
        .method(Method::POST)
        .uri(format!("/api/mcp?workspace_id={}", ctx.workspace_id))
        .header(AUTHORIZATION, format!("Bearer {}", ctx.state.api_token))
        .header("content-type", "application/json")
        .header("x-opendesk-mcp-role", "read")
        .body(axum::body::Body::from(
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/list",
                "params": {}
            })
            .to_string(),
        ))
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(request)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    let body: Value = serde_json::from_slice(&bytes).expect("json");
    let tools = body["result"]["tools"].as_array().expect("tools");
    assert!(tools.iter().any(|tool| tool["name"] == "task_list"));
    assert!(!tools.iter().any(|tool| tool["name"] == "task_create"));
}
