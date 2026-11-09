use super::*;

#[tokio::test]
async fn browse_activation_writes_mcp_and_skill() {
    let ctx = test_support::context().await;
    let workspace = format!("workspace_id={}", ctx.workspace_id);

    let (status, body) = call(
        &ctx,
        Method::GET,
        &format!("/api/browse/status?{workspace}&scope=workspace"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["configured"], false);
    assert_eq!(body["skill"], false);
    assert!(!body["command"].as_array().expect("command").is_empty());

    let (status, enabled) = call(
        &ctx,
        Method::POST,
        "/api/browse/enable",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "scope": "workspace"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "enable: {enabled}");
    assert_eq!(enabled["configured"], true);
    assert_eq!(enabled["skill"], true);
    assert!(
        ctx.workspace_root
            .join(".opencode/skills/browse-web/SKILL.md")
            .is_file()
    );
    let config =
        std::fs::read_to_string(ctx.workspace_root.join("opencode.jsonc")).expect("config");
    assert!(config.contains("\"browse\""));

    let (status, disabled) = call(
        &ctx,
        Method::POST,
        "/api/browse/disable",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "scope": "workspace"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(disabled["configured"], false);
    assert_eq!(disabled["skill"], true);
}
