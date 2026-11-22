use crate::context::RecipeContext;
use crate::parser;
use crate::store;

use opendesk_storage::events::EventBus;
use opendesk_storage::repos::tasks;
use opendesk_storage::secrets::SecretStore;
use serde_json::json;
use std::sync::Arc;

mod execution;
mod recipes;
mod resume;

fn recipe_context(ctx: &crate::test_support::TestContext) -> RecipeContext {
    RecipeContext::new(
        ctx.db.clone(),
        ctx.paths.clone(),
        EventBus::new(32),
        Arc::new(SecretStore::new(&ctx.paths)),
    )
}

const YAML: &str = r#"
id: dag-exec
name: DAG exec
flow:
  - type: branch
    id: decidir
    when: "true"
    then:
      - type: step
        id: t1
        name: Then
        prompt: a
        when: "false"
    else:
      - type: step
        id: e1
        name: Else
        prompt: b
        when: "false"
  - type: parallel
    id: lote
    join: all
    branches:
      - - type: step
          id: p1
          name: P1
          prompt: a
          when: "false"
      - - type: step
          id: p2
          name: P2
          prompt: b
          when: "false"
  - type: map
    id: arquivos
    items: "[]"
    flow:
      - type: step
        id: analisar
        name: Analisar
        prompt: c
"#;

async fn fixture_with(
    yaml: &str,
    id: &str,
) -> (
    crate::test_support::TestContext,
    parser::Recipe,
    store::RecipeRun,
) {
    let ctx = crate::test_support::context().await;
    store::save_yaml(&ctx.db, &ctx.workspace_id, id, yaml)
        .await
        .expect("save");
    let recipe = parser::parse(yaml).expect("recipe");
    let task = tasks::create(
        &ctx.db,
        &ctx.workspace_id,
        tasks::CreateTaskInput {
            title: "dag".into(),
            description: "dag".into(),
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
        "/tmp/dag-out",
        false,
    )
    .await
    .expect("run");
    (ctx, recipe, run)
}

async fn fixture() -> (
    crate::test_support::TestContext,
    parser::Recipe,
    store::RecipeRun,
) {
    fixture_with(YAML, "dag-exec").await
}
