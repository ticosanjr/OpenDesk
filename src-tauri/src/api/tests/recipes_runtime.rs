use super::*;
use crate::recipes::{parser, store};

#[tokio::test]
async fn versions_diff_and_rollback_a_recipe_through_the_api() {
    let ctx = test_support::context().await;
    let v1 = "id: api-version\nname: API Version\nflow:\n  - type: step\n    id: s1\n    name: S1\n    prompt: v1\n";
    let v2 = "id: api-version\nname: API Version\nflow:\n  - type: step\n    id: s1\n    name: S1\n    prompt: v2\n";
    let path = format!("/api/recipes/api-version?workspace_id={}", ctx.workspace_id);
    let (status, saved) = call(&ctx, Method::PUT, &path, Some(json!({ "yaml": v1 }))).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let (status, _) = call(&ctx, Method::PUT, &path, Some(json!({ "yaml": v2 }))).await;
    assert_eq!(status, StatusCode::OK);

    let versions_path = format!(
        "/api/recipes/api-version/versions?workspace_id={}",
        ctx.workspace_id
    );
    let (status, versions) = call(&ctx, Method::GET, &versions_path, None).await;
    assert_eq!(status, StatusCode::OK, "{versions}");
    let list = versions.as_array().expect("versions");
    assert!(list.len() >= 2, "{versions}");
    let oldest = list.last().expect("oldest")["id"]
        .as_str()
        .expect("id")
        .to_string();

    let diff_path = format!(
        "/api/recipes/api-version/versions/{oldest}/diff?workspace_id={}",
        ctx.workspace_id
    );
    let (status, diff) = call(&ctx, Method::GET, &diff_path, None).await;
    assert_eq!(status, StatusCode::OK, "{diff}");
    assert!(diff["diff"].as_str().unwrap_or_default().contains("v2"));

    let rollback_path = format!(
        "/api/recipes/api-version/rollback?workspace_id={}",
        ctx.workspace_id
    );
    let (status, rolled) = call(
        &ctx,
        Method::POST,
        &rollback_path,
        Some(json!({ "version_id": oldest })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{rolled}");
    let (status, current) = call(&ctx, Method::GET, &path, None).await;
    assert_eq!(status, StatusCode::OK, "{current}");
    assert!(
        current["yaml"].as_str().unwrap_or_default().contains("v1"),
        "{current}"
    );
}

#[tokio::test]
async fn cancels_a_durable_run_and_exposes_nodes() {
    let ctx = test_support::context().await;
    let yaml = "id: api-durable\nname: API Durable\ntrigger:\n  type: MANUAL\n  config: {}\nflow:\n  - type: step\n    id: s1\n    name: Passo 1\n    prompt: oi\n  - type: sink\n    sink_type: DOC_COMPILER\n    config: {}\n";
    store::save_yaml(&ctx.state.db, &ctx.workspace_id, "api-durable", yaml)
        .await
        .expect("recipe");
    let recipe = parser::parse(yaml).expect("parse");
    let run = store::create_run(
        &ctx.state.db,
        &recipe,
        &ctx.workspace_id,
        None,
        &json!({}),
        "/tmp/out",
        false,
    )
    .await
    .expect("run");
    store::sync_run_nodes(&ctx.state.db, &run.id, &recipe.flow)
        .await
        .expect("nodes");
    store::mark_node_running(&ctx.state.db, &run.id, 0)
        .await
        .expect("running");

    let (status, detail) = call(
        &ctx,
        Method::GET,
        &format!("/api/recipes/runs/{}", run.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert_eq!(detail["nodes"].as_array().map(Vec::len), Some(2));

    let (status, cancelled) = call(
        &ctx,
        Method::POST,
        &format!("/api/recipes/runs/{}/cancel", run.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cancelled}");
    assert_eq!(cancelled["cancel_requested"], true);

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/recipes/runs/{}/cancel", run.id),
        None,
    )
    .await;
    // Still active until the driver observes the flag, so a second cancel is
    // accepted; terminal runs are rejected elsewhere.
    assert!(matches!(status, StatusCode::OK | StatusCode::CONFLICT));
}
