use super::*;

#[tokio::test]
async fn installs_toggles_tests_and_deletes_a_catalog_server() {
    let ctx = test_support::context().await;
    let (status, catalog) = call(&ctx, Method::GET, "/api/mcp-hub/catalog", None).await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    assert_eq!(catalog.as_array().map(Vec::len), Some(5));

    let (status, installed) = call(
        &ctx,
        Method::POST,
        "/api/mcp-hub/install",
        Some(json!({ "catalog_id": "fetch", "scope": "workspace" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{installed}");
    assert_eq!(installed["name"], "fetch");
    assert_eq!(installed["scope"], "workspace");
    assert_eq!(installed["transport"], "local");

    let (_, servers) = call(&ctx, Method::GET, "/api/mcp-hub/servers", None).await;
    assert!(
        servers
            .as_array()
            .expect("servers")
            .iter()
            .any(|server| server["name"] == "fetch" && server["scope"] == "workspace")
    );

    let (status, toggled) = call(
        &ctx,
        Method::POST,
        "/api/mcp-hub/workspace/fetch/toggle",
        Some(json!({ "disabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{toggled}");
    let (_, servers) = call(&ctx, Method::GET, "/api/mcp-hub/servers", None).await;
    let fetch = servers
        .as_array()
        .expect("servers")
        .iter()
        .find(|server| server["name"] == "fetch")
        .expect("fetch");
    assert_eq!(fetch["disabled"], true);
    assert_eq!(fetch["health"], "disabled");

    let (status, result) = call(
        &ctx,
        Method::POST,
        "/api/mcp-hub/workspace/fetch/test",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["status"], "disabled");

    let (status, _) = call(&ctx, Method::DELETE, "/api/mcp-hub/workspace/fetch", None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn rejects_unknown_catalog_entries_and_scopes() {
    let ctx = test_support::context().await;
    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/mcp-hub/install",
        Some(json!({ "catalog_id": "nope", "scope": "workspace" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/mcp-hub/install",
        Some(json!({ "catalog_id": "fetch", "scope": "elsewhere" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
