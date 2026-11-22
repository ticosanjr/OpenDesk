use crate::dag::execute;
use crate::store;
use serde_json::{Value, json};

use super::{fixture, recipe_context};

#[tokio::test]
async fn resume_skips_a_completed_step_and_restores_its_output() {
    let (ctx, recipe, run) = fixture().await;
    store::sync_run_nodes(&ctx.db, &run.id, &recipe.flow)
        .await
        .expect("sync");
    store::mark_node_path(
        &ctx.db,
        &run.id,
        "decidir/t1",
        "DONE",
        Some(&json!({ "summary": "cached" })),
        Some("session-1"),
        None,
    )
    .await
    .expect("cache");

    execute(
        &recipe_context(&ctx),
        &recipe,
        &run,
        &json!({}),
        &opendesk_engine::recipe_dispatch::NoopRecipeDispatcher,
    )
    .await
    .expect("execute");

    let node = store::node_by_path(&ctx.db, &run.id, "decidir/t1")
        .await
        .expect("node")
        .expect("row");
    assert_eq!(node.status, "DONE");
    assert_eq!(node.attempts, 0, "resume must not re-run a DONE step");
    let completed = store::get_run(&ctx.db, &run.id).await.expect("run");
    let outputs: Value = serde_json::from_str(&completed.step_outputs_json).expect("outputs");
    assert_eq!(outputs["t1"]["summary"], "cached");
}
