use super::*;

#[tokio::test]
async fn sync_manifest_bundle_and_apply_roundtrip() {
    let ctx = test_support::context().await;
    std::fs::create_dir_all(ctx.workspace_root.join("input")).expect("input");
    std::fs::write(ctx.workspace_root.join("input/sync.txt"), "hello").expect("file");
    let base = format!("/api/workspaces/{}/sync", ctx.workspace_id);

    let (status, manifest) = call(&ctx, Method::GET, &format!("{base}/manifest"), None).await;
    assert_eq!(status, StatusCode::OK, "{manifest}");
    assert!(
        manifest["files"]
            .as_array()
            .expect("files")
            .iter()
            .any(|file| file["path"] == "input/sync.txt"),
        "{manifest}"
    );

    let (status, bundle) = call(
        &ctx,
        Method::POST,
        &format!("{base}/bundle"),
        Some(json!({ "base": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{bundle}");
    assert!(bundle["bundle"]["files"].as_u64().unwrap_or(0) >= 1);
    let path = bundle["bundle"]["path"].as_str().expect("bundle path");

    let (status, dry) = call(
        &ctx,
        Method::POST,
        &format!("{base}/apply"),
        Some(json!({ "bundle_path": path, "dry_run": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{dry}");
    assert_eq!(dry["report"]["dry_run"], true);

    let (status, applied) = call(
        &ctx,
        Method::POST,
        &format!("{base}/apply"),
        Some(json!({ "bundle_path": path })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert!(applied["report"]["files"].as_u64().unwrap_or(0) >= 1);
    assert_eq!(
        std::fs::read_to_string(ctx.workspace_root.join("input/sync.txt")).expect("file"),
        "hello"
    );
}
