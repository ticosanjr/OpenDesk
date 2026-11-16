use super::*;
use crate::recipes::{parser, store};

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
