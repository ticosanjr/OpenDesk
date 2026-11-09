use super::*;
use crate::recipes::store as recipe_store;

#[tokio::test]
async fn attention_returns_human_questions_and_hides_automation_tasks() {
    let ctx = test_support::context().await;
    let workspace = format!("?workspace_id={}", ctx.workspace_id);

    let yaml = "id: r1\nname: Receita de atenção\ntrigger:\n  type: MANUAL\n  config: {}\nflow:\n  - type: step\n    id: s1\n    name: Passo\n    prompt: oi\n";
    let recipe = crate::recipes::parser::parse(yaml).expect("recipe");
    sqlx::query(
        "INSERT INTO recipes_index (id, workspace_id, name, file_path, trigger_type)
         VALUES ('r1', ?, 'Receita de atenção', '/tmp/r1.yaml', 'MANUAL')",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.state.db)
    .await
    .expect("index");
    let run = recipe_store::create_run(
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
    recipe_store::create_hitl(
        &ctx.state.db,
        "TOKEN",
        &run,
        "checkpoint-1",
        "BINARY_GATE",
        "Receita de atenção",
        "Aprovar o envio do relatório?",
        &json!({ "next_step": "Publicar" }),
        &[],
        "PAUSE_AND_ALERT",
        "2999-01-01T00:00:00Z",
    )
    .await
    .expect("hitl");

    sqlx::query(
        "INSERT INTO tasks (id, workspace_id, title, description, status, origin, plan_markdown)
         VALUES ('manual-1', ?, 'Manual bloqueada', 'x', 'BLOCKED', 'MANUAL', NULL)",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.state.db)
    .await
    .expect("manual");
    sqlx::query(
        "INSERT INTO tasks (id, workspace_id, title, description, status, origin)
         VALUES ('recipe-1', ?, 'Receita bloqueada', 'x', 'BLOCKED', 'RECIPE')",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.state.db)
    .await
    .expect("recipe task");

    let (status, body) = call(
        &ctx,
        Method::GET,
        &format!("/api/attention{workspace}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["hitl"][0]["title"], "Receita de atenção");
    assert_eq!(
        body["hitl"][0]["question_markdown"],
        "Aprovar o envio do relatório?"
    );
    assert!(
        body["hitl"][0]["context_json"]
            .as_str()
            .unwrap_or_default()
            .contains("Publicar")
    );

    let blocked = body["blocked"].as_array().expect("blocked");
    assert!(blocked.iter().any(|task| task["id"] == "manual-1"));
    assert!(!blocked.iter().any(|task| task["id"] == "recipe-1"));
}

#[tokio::test]
async fn plan_approval_exposes_a_plan_excerpt() {
    let ctx = test_support::context().await;
    sqlx::query(
        "INSERT INTO tasks (id, workspace_id, title, description, status, origin, phase_status, plan_markdown)
         VALUES ('plan-1', ?, 'Plano', 'x', 'IN_REVIEW', 'MANUAL', 'awaiting_approval', '# Plano\n\n1. Fazer')",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.state.db)
    .await
    .expect("task");

    let (status, body) = call(
        &ctx,
        Method::GET,
        &format!("/api/attention?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["awaiting_approval"][0]["id"], "plan-1");
    assert!(
        body["awaiting_approval"][0]["plan_excerpt"]
            .as_str()
            .unwrap_or_default()
            .contains("Plano")
    );
}
