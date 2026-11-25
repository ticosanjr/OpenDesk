use super::*;

#[tokio::test]
async fn fast_self_test_passes_offline_and_cleans_everything() {
    let ctx = test_support::context().await;
    let (status, report) = call(
        &ctx,
        Method::POST,
        "/api/diagnostics/self-test",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "mode": "fast"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert_eq!(report["mode"], "fast");
    assert_eq!(report["ok"], true, "{report}");
    let steps = report["steps"].as_array().expect("steps");
    let ids = steps
        .iter()
        .filter_map(|step| step["id"].as_str())
        .collect::<Vec<_>>();
    for expected in [
        "db",
        "engine",
        "task",
        "routine",
        "recipe",
        "script_tool",
        "voice",
        "cleanup",
    ] {
        assert!(ids.contains(&expected), "missing {expected}: {report}");
    }
    assert_eq!(
        steps
            .iter()
            .find(|step| step["id"] == "cleanup")
            .map(|step| step["status"].as_str()),
        Some(Some("pass")),
        "{report}"
    );

    let tasks: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks WHERE workspace_id = ? AND title LIKE '__selftest_%'",
    )
    .bind(&ctx.workspace_id)
    .fetch_one(&ctx.state.db)
    .await
    .expect("tasks");
    let schedules: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM schedules WHERE workspace_id = ? AND name LIKE '__selftest_%'",
    )
    .bind(&ctx.workspace_id)
    .fetch_one(&ctx.state.db)
    .await
    .expect("schedules");
    let recipes: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM recipes_index WHERE workspace_id = ? AND id LIKE '__selftest_%'",
    )
    .bind(&ctx.workspace_id)
    .fetch_one(&ctx.state.db)
    .await
    .expect("recipes");
    let scripts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM script_tools WHERE workspace_id = ? AND name LIKE '__selftest_%'",
    )
    .bind(&ctx.workspace_id)
    .fetch_one(&ctx.state.db)
    .await
    .expect("scripts");
    assert_eq!(tasks + schedules + recipes + scripts, 0);

    let recipes_dir = ctx.workspace_root.join(".opencode/recipes");
    if let Ok(entries) = std::fs::read_dir(recipes_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            assert!(
                !name.starts_with("__selftest_"),
                "leftover recipe file: {name}"
            );
        }
    }
}
