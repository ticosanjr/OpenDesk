use crate::dag::execute;
use crate::store;
use serde_json::{Value, json};

use super::{fixture, fixture_with, recipe_context};

#[tokio::test]
async fn executes_switch_and_bounded_loop_with_durable_paths() {
    let yaml = r#"
id: loop-switch
name: Loop
flow:
  - type: switch
    id: escolher
    cases:
      - when: "true"
        flow:
          - type: step
            id: s1
            name: S1
            prompt: a
            when: "false"
      - when: "false"
        flow:
          - type: step
            id: s2
            name: S2
            prompt: b
            when: "false"
    default:
      - type: step
        id: s3
        name: S3
        prompt: c
        when: "false"
  - type: loop
    id: repetir
    max_iterations: 2
    flow:
      - type: step
        id: l1
        name: L1
        prompt: d
        when: "false"
"#;
    let (ctx, recipe, run) = fixture_with(yaml, "loop-switch").await;
    execute(
        &recipe_context(&ctx),
        &recipe,
        &run,
        &json!({}),
        &opendesk_engine::recipe_dispatch::NoopRecipeDispatcher,
    )
    .await
    .expect("execute");
    let completed = store::get_run(&ctx.db, &run.id).await.expect("run");
    assert_eq!(completed.status, "COMPLETED");
    for path in [
        "escolher",
        "escolher/s1",
        "repetir",
        "repetir/0",
        "repetir/0/l1",
    ] {
        assert!(
            store::node_by_path(&ctx.db, &run.id, path)
                .await
                .expect("node")
                .is_some(),
            "missing {path}"
        );
    }
}

#[tokio::test]
async fn executes_branch_parallel_and_empty_map_without_dispatching() {
    let (ctx, recipe, run) = fixture().await;
    execute(
        &recipe_context(&ctx),
        &recipe,
        &run,
        &json!({}),
        &opendesk_engine::recipe_dispatch::NoopRecipeDispatcher,
    )
    .await
    .expect("execute");

    let completed = store::get_run(&ctx.db, &run.id).await.expect("run");
    assert_eq!(completed.status, "COMPLETED");
    let outputs: Value = serde_json::from_str(&completed.step_outputs_json).expect("outputs");
    assert_eq!(outputs["decidir"]["then"], true);
    assert_eq!(outputs["arquivos"]["count"], 0);

    let nodes = store::list_run_nodes(&ctx.db, &run.id)
        .await
        .expect("nodes");
    for expected in [
        ("decidir", "DONE"),
        ("decidir/t1", "SKIPPED"),
        ("decidir/e1", "SKIPPED"),
        ("lote", "DONE"),
        ("lote/b0/p1", "SKIPPED"),
        ("lote/b1/p2", "SKIPPED"),
        ("arquivos", "DONE"),
    ] {
        let node = nodes
            .iter()
            .find(|node| node.node_path.as_deref() == Some(expected.0))
            .unwrap_or_else(|| panic!("missing {}", expected.0));
        assert_eq!(node.status, expected.1, "node {}", expected.0);
    }
}

#[tokio::test]
async fn expands_map_items_into_durable_paths() {
    let yaml = r#"
id: dag-map
name: DAG map
flow:
  - type: map
    id: mapa
    items: '[{"path":"a.txt"},{"path":"b.txt"}]'
    max_concurrency: 2
    flow:
      - type: step
        id: analisar
        name: Analisar
        prompt: "Analise {{item.path}}"
        when: "false"
"#;
    let (ctx, recipe, run) = fixture_with(yaml, "dag-map").await;
    execute(
        &recipe_context(&ctx),
        &recipe,
        &run,
        &json!({}),
        &opendesk_engine::recipe_dispatch::NoopRecipeDispatcher,
    )
    .await
    .expect("execute");

    let completed = store::get_run(&ctx.db, &run.id).await.expect("run");
    let outputs: Value = serde_json::from_str(&completed.step_outputs_json).expect("outputs");
    assert_eq!(outputs["mapa"]["count"], 2);
    assert_eq!(outputs["mapa"]["items"][0]["key"], "0000-a");
    assert_eq!(outputs["mapa"]["items"][1]["key"], "0001-b");

    let nodes = store::list_run_nodes(&ctx.db, &run.id)
        .await
        .expect("nodes");
    for path in [
        "mapa/0000-a",
        "mapa/0000-a/analisar",
        "mapa/0001-b",
        "mapa/0001-b/analisar",
    ] {
        assert!(
            nodes
                .iter()
                .any(|node| node.node_path.as_deref() == Some(path)),
            "missing node {path}"
        );
    }
}
