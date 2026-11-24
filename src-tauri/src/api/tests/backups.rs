use super::*;

#[tokio::test]
async fn workspace_backups_create_list_restore_and_delete() {
    let ctx = test_support::context().await;
    std::fs::create_dir_all(ctx.workspace_root.join("input")).expect("input");
    std::fs::write(ctx.workspace_root.join("input/brief.md"), "brief").expect("brief");
    let workspace = format!("workspace_id={}", ctx.workspace_id);

    let (status, created) = call(
        &ctx,
        Method::POST,
        &format!("/api/backups?{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create: {created}");
    let id = created["id"].as_str().expect("backup id");
    assert_eq!(created["files"], 1);
    assert!(created["sha256"].as_str().unwrap_or_default().len() == 64);

    let (status, list) = call(
        &ctx,
        Method::GET,
        &format!("/api/backups?{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list[0]["id"], id);

    let target = ctx.temp.path().join("restored-api");
    let (status, restored) = call(
        &ctx,
        Method::POST,
        &format!("/api/backups/{id}/restore?{workspace}"),
        Some(json!({
            "target_path": target.to_string_lossy().to_string(),
            "name": "Restaurado via API"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "restore: {restored}");
    assert_eq!(restored["name"], "Restaurado via API");
    assert!(target.join("input/brief.md").is_file());

    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/backups/{id}?{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, list) = call(
        &ctx,
        Method::GET,
        &format!("/api/backups?{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(list.as_array().expect("list").is_empty());
}

#[tokio::test]
async fn workspace_backup_accepts_log_and_secret_options() {
    let ctx = test_support::context().await;
    let workspace = format!("workspace_id={}", ctx.workspace_id);
    let (status, created) = call(
        &ctx,
        Method::POST,
        &format!("/api/backups?{workspace}"),
        Some(json!({ "include_logs": false, "passphrase": "segredo-forte" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "create: {created}");
    assert_eq!(created["version"], 2);
    assert_eq!(created["includes"]["encrypted_secrets"], true);
    assert_eq!(created["includes"]["run_logs"], false);
    assert!(created["counts"]["files"].is_number());
}
