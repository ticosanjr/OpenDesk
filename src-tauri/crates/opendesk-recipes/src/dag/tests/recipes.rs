use crate::dag::execute;
use crate::parser;
use crate::store;

use opendesk_storage::repos::tasks;
use serde_json::{Value, json};

use super::recipe_context;

#[tokio::test]
async fn calls_a_sub_recipe_with_isolated_outputs() {
    let child = r#"
id: dag-child
name: Child
flow:
  - type: step
    id: child-step
    name: Child step
    prompt: "{{trigger.topic}}"
    when: "false"
"#;
    let parent = r#"
id: dag-parent
name: Parent
flow:
  - type: recipe
    id: chamar
    recipe: dag-child
    with:
      topic: "assunto"
  - type: step
    id: after
    name: After
    prompt: x
    when: "false"
"#;
    let ctx = crate::test_support::context().await;
    store::save_yaml(&ctx.db, &ctx.workspace_id, "dag-child", child)
        .await
        .expect("child");
    let (ctx, recipe, run) = {
        let saved = store::save_yaml(&ctx.db, &ctx.workspace_id, "dag-parent", parent)
            .await
            .expect("parent");
        let recipe = parser::parse(parent).expect("recipe");
        let task = tasks::create(
            &ctx.db,
            &ctx.workspace_id,
            tasks::CreateTaskInput {
                title: "parent".into(),
                description: "parent".into(),
                execution_mode: Some("BUILD_DIRECT".into()),
                priority: Some(2),
                assigned_agent: None,
                preferred_model: None,
                preferred_harness: None,
                effort_level: None,
                care_level: Some("standard".into()),
                notify_email: None,
                export_to_gdocs: false,
                tags: vec![],
                origin: Some("RECIPE".into()),
                parent_task_id: None,
                dedupe: None,
            },
        )
        .await
        .expect("task");
        let run = store::create_run(
            &ctx.db,
            &recipe,
            &ctx.workspace_id,
            Some(&task.id),
            &json!({}),
            "/tmp/dag-parent",
            false,
        )
        .await
        .expect("run");
        let _ = saved;
        (ctx, recipe, run)
    };
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
    assert_eq!(outputs["chamar"]["recipe"], "dag-child");
    assert_eq!(outputs["chamar"]["outputs"]["child-step"]["skipped"], true);
    let nodes = store::list_run_nodes(&ctx.db, &run.id)
        .await
        .expect("nodes");
    assert!(
        nodes.iter().any(
            |node| node.node_path.as_deref() == Some("chamar/child-step")
                && node.status == "SKIPPED"
        ),
        "{nodes:?}"
    );
}

#[tokio::test]
async fn rejects_sub_recipe_cycles() {
    let a = r#"
id: cycle-a
name: A
flow:
  - type: recipe
    id: call-b
    recipe: cycle-b
  - type: step
    id: a-step
    name: A step
    prompt: x
    when: "false"
"#;
    let b = r#"
id: cycle-b
name: B
flow:
  - type: recipe
    id: call-a
    recipe: cycle-a
  - type: step
    id: b-step
    name: B step
    prompt: x
    when: "false"
"#;
    let ctx = crate::test_support::context().await;
    store::save_yaml(&ctx.db, &ctx.workspace_id, "cycle-b", b)
        .await
        .expect("b");
    let recipe = parser::parse(a).expect("a");
    store::save_yaml(&ctx.db, &ctx.workspace_id, "cycle-a", a)
        .await
        .expect("a index");
    let task = tasks::create(
        &ctx.db,
        &ctx.workspace_id,
        tasks::CreateTaskInput {
            title: "cycle".into(),
            description: "cycle".into(),
            execution_mode: Some("BUILD_DIRECT".into()),
            priority: Some(2),
            assigned_agent: None,
            preferred_model: None,
            preferred_harness: None,
            effort_level: None,
            care_level: Some("standard".into()),
            notify_email: None,
            export_to_gdocs: false,
            tags: vec![],
            origin: Some("RECIPE".into()),
            parent_task_id: None,
            dedupe: None,
        },
    )
    .await
    .expect("task");
    let run = store::create_run(
        &ctx.db,
        &recipe,
        &ctx.workspace_id,
        Some(&task.id),
        &json!({}),
        "/tmp/cycle",
        false,
    )
    .await
    .expect("run");
    let error = execute(
        &recipe_context(&ctx),
        &recipe,
        &run,
        &json!({}),
        &opendesk_engine::recipe_dispatch::NoopRecipeDispatcher,
    )
    .await
    .expect_err("cycle");
    assert!(format!("{error:#}").contains("ciclo"), "{error:#}");
}
