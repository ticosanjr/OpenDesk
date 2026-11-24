use super::*;

#[tokio::test]
async fn config_bundle_exports_previews_and_imports() {
    let ctx = test_support::context().await;
    let workspace = format!("workspace_id={}", ctx.workspace_id);

    let (status, bundle) = call(
        &ctx,
        Method::POST,
        "/api/config-bundle/export",
        Some(json!({ "workspace_id": ctx.workspace_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "export: {bundle}");
    assert_eq!(bundle["schema_version"], 1);

    let (status, preview) = call(
        &ctx,
        Method::POST,
        "/api/config-bundle/preview",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "bundle": bundle,
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "preview: {preview}");
    assert!(preview["sections"].is_object());

    let (status, imported) = call(
        &ctx,
        Method::POST,
        "/api/config-bundle/import",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "bundle": bundle,
            "mode": "merge-skip-existing",
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "import: {imported}");

    let (status, invalid) = call(
        &ctx,
        Method::POST,
        "/api/config-bundle/preview",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "bundle": { "schema_version": 99, "exported_at": "", "app_version": "", "source_workspace": {}, "sections": {} },
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{invalid}");
    assert!(workspace.contains("workspace_id"));
}
