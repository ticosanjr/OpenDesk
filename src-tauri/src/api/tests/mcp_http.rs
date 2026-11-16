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
