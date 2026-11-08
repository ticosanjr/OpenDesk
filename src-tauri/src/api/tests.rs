use super::*;
use crate::test_support::{self, TestContext};
use axum::{
    body::{Body, to_bytes},
    http::{Method, Request, StatusCode, header::AUTHORIZATION},
};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn call(
    ctx: &TestContext,
    method: Method,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(AUTHORIZATION, format!("Bearer {}", ctx.state.api_token));
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let request = builder
        .body(
            body.map(|value| Body::from(value.to_string()))
                .unwrap_or_else(Body::empty),
        )
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(request)
        .await
        .expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).to_string()))
    };
    (status, value)
}

async fn unauthenticated(ctx: &TestContext, uri: &str) -> StatusCode {
    let request = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("request");
    router(ctx.state.clone())
        .oneshot(request)
        .await
        .expect("response")
        .status()
}

#[tokio::test]
async fn health_is_public_but_api_requires_bearer_token() {
    let ctx = test_support::context().await;
    assert_eq!(unauthenticated(&ctx, "/healthz").await, StatusCode::OK);
    assert_eq!(
        unauthenticated(&ctx, "/api/workspaces").await,
        StatusCode::UNAUTHORIZED
    );
    let (status, _) = call(&ctx, Method::GET, "/api/health", None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn starter_pack_api_previews_installs_and_removes() {
    let ctx = test_support::context().await;
    let (status, catalog) = call(&ctx, Method::GET, "/api/starter-packs", None).await;
    assert_eq!(status, StatusCode::OK);
    let packs = catalog["packs"].as_array().expect("packs");
    assert!(packs.len() >= 20);
    assert!(packs.iter().all(|pack| pack["installed"] == false));

    let (status, preview) = call(
        &ctx,
        Method::POST,
        "/api/starter-packs/essenciais/preview",
        Some(json!({ "workspace_id": ctx.workspace_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!preview["files"].as_array().unwrap().is_empty());

    let (status, installed) = call(
        &ctx,
        Method::POST,
        "/api/starter-packs/essenciais/install",
        Some(json!({ "workspace_id": ctx.workspace_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(installed["written"].as_u64().unwrap() > 0);
    assert!(
        ctx.workspace_root
            .join(".opencode/agents/assistente.md")
            .is_file()
    );

    let (status, detail) = call(
        &ctx,
        Method::GET,
        &format!(
            "/api/starter-packs/essenciais?workspace_id={}",
            ctx.workspace_id
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["installed"], true);
    assert_eq!(detail["health"]["status"], "installed");

    let (status, removed) = call(
        &ctx,
        Method::DELETE,
        &format!(
            "/api/starter-packs/essenciais?workspace_id={}",
            ctx.workspace_id
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(removed["removed"].as_u64().unwrap() > 0);
    assert!(
        !ctx.workspace_root
            .join(".opencode/agents/assistente.md")
            .exists()
    );
}

#[tokio::test]
async fn starter_pack_conflict_requires_explicit_overwrite() {
    let ctx = test_support::context().await;
    let agent = ctx.workspace_root.join(".opencode/agents/assistente.md");
    std::fs::create_dir_all(agent.parent().unwrap()).expect("agents dir");
    std::fs::write(&agent, "conteúdo do usuário").expect("custom agent");

    let (_, preview) = call(
        &ctx,
        Method::POST,
        "/api/starter-packs/essenciais/preview",
        Some(json!({ "workspace_id": ctx.workspace_id })),
    )
    .await;
    assert_eq!(preview["conflicts"], 1);

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/starter-packs/essenciais/install",
        Some(json!({ "workspace_id": ctx.workspace_id })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/starter-packs/essenciais/install",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "overwrite": [".opencode/agents/assistente.md"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        std::fs::read_to_string(&agent)
            .expect("agent")
            .contains("Assistente")
    );
}

#[tokio::test]
async fn workspace_template_creates_workspace_and_installs_packs() {
    let ctx = test_support::context().await;
    let root = ctx.temp.path().join("template-workspace");
    let (status, result) = call(
        &ctx,
        Method::POST,
        "/api/starter-packs/templates/create",
        Some(json!({
            "template_id": "conteudo",
            "name": "Conteúdo API",
            "root_path": root.to_string_lossy(),
            "color": "#8b5cf6"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["errors"].as_array().unwrap().len(), 0);
    assert_eq!(result["installed"].as_array().unwrap().len(), 5);
    assert!(
        root.join(".opencode/agents/roteirista-youtube.md")
            .is_file()
    );
    assert!(root.join("README.md").is_file());

    let (status, workspaces) = call(&ctx, Method::GET, "/api/workspaces", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        workspaces
            .as_array()
            .unwrap()
            .iter()
            .any(|workspace| workspace["name"] == "Conteúdo API")
    );
}

#[tokio::test]
async fn studio_creates_agent_skill_and_command() {
    let ctx = test_support::context().await;
    let auth_query = format!("scope=workspace&workspace_id={}", ctx.workspace_id);

    let (status, agent) = call(
        &ctx,
        Method::PUT,
        &format!("/api/studio/agent?id=auditor&{auth_query}"),
        Some(json!({
            "name": "Auditor",
            "description": "Revisa qualidade",
            "mode": "subagent",
            "system": "Você revisa."
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(agent["name"], "Auditor");

    let (status, skill) = call(
        &ctx,
        Method::PUT,
        &format!("/api/studio/skill?id=revisao&{auth_query}"),
        Some(json!({
            "name": "Revisão",
            "description": "Revisa entregáveis com checklist",
            "body": "## Processo\n1. Leia.\n2. Verifique.",
            "files": [{ "path": "references/checklist.md", "content": "- item" }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(skill["files"].as_array().unwrap().len(), 1);

    let (status, command) = call(
        &ctx,
        Method::PUT,
        &format!("/api/studio/command?id=revisar&{auth_query}"),
        Some(json!({
            "description": "Revisa um texto",
            "agent": "auditor",
            "template": "Revise: $ARGUMENTS"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(command["template"].as_str().unwrap().contains("$ARGUMENTS"));
}

#[tokio::test]
async fn files_search_finds_nested_files() {
    let ctx = test_support::context().await;
    let nested = ctx.workspace_root.join("output/2026-11/relatorios");
    std::fs::create_dir_all(&nested).expect("nested");
    std::fs::write(nested.join("fechamento-final.md"), "# ok").expect("file");
    let (status, results) = call(
        &ctx,
        Method::GET,
        &format!(
            "/api/files/search?workspace_id={}&q=fechamento",
            ctx.workspace_id
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let results = results.as_array().expect("array");
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0]["path"],
        "output/2026-11/relatorios/fechamento-final.md"
    );
}

#[tokio::test]
async fn attention_dismissal_is_versioned() {
    let ctx = test_support::context().await;
    sqlx::query(
        "INSERT INTO tasks (id, workspace_id, title, description, status, phase_status, updated_at)
         VALUES ('task-attention', ?, 'Precisa aprovar', 'teste', 'IN_REVIEW', 'awaiting_approval', '2026-01-01T00:00:00Z')",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.state.db)
    .await
    .expect("task");

    let (status, attention) = call(
        &ctx,
        Method::GET,
        &format!("/api/attention?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let approval = &attention["awaiting_approval"][0];
    assert_eq!(approval["id"], "task-attention");
    let version = approval["version"].as_str().expect("version").to_string();

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/attention/dismiss",
        Some(json!({
            "kind": "APPROVAL",
            "ref_id": "task-attention",
            "version": version,
            "workspace_id": ctx.workspace_id
        })),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (_, attention) = call(
        &ctx,
        Method::GET,
        &format!("/api/attention?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert!(
        attention["awaiting_approval"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    sqlx::query("UPDATE tasks SET updated_at = '2026-01-02T00:00:00Z' WHERE id = 'task-attention'")
        .execute(&ctx.state.db)
        .await
        .expect("touch");
    let (_, attention) = call(
        &ctx,
        Method::GET,
        &format!("/api/attention?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(attention["awaiting_approval"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn recipe_webhook_trigger_creates_an_inbound_source() {
    let ctx = test_support::context().await;
    let yaml = r#"
id: webhook-api
name: Webhook API
triggers:
  - type: WEBHOOK
    config: { secret: teste }
flow:
  - type: step
    id: s1
    name: Um
    prompt: ok
"#;
    let (status, _) = call(
        &ctx,
        Method::PUT,
        &format!("/api/recipes/webhook-api?workspace_id={}", ctx.workspace_id),
        Some(json!({ "yaml": yaml })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, sources) = call(
        &ctx,
        Method::GET,
        &format!("/api/io/inbound?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let sources = sources["sources"].as_array().expect("sources");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0]["source_type"], "WEBHOOK_HTTP");
    let config: Value =
        serde_json::from_str(sources[0]["config_json"].as_str().unwrap()).expect("config");
    assert_eq!(config["recipe_id"], "webhook-api");
}

#[tokio::test]
async fn routine_source_is_created_and_removed_by_api() {
    let ctx = test_support::context().await;
    let routine = json!({
        "workspace_id": ctx.workspace_id,
        "name": "Rotina pasta",
        "description": "teste",
        "cron_expression": "",
        "timezone": "America/Sao_Paulo",
        "agent_name": null,
        "command_or_prompt": "teste",
        "trigger_type": "FOLDER_WATCH",
        "trigger_config_json": "{\"path\":\"input\"}",
        "target_kind": "PROMPT",
        "recipe_id": null,
        "router_policy": "COST_BENEFIT",
        "output_folder_template": "output/{YYYY}-{MM}/{schedule_slug}",
        "notify_email_recipients": null,
        "notify_signal": false,
        "notify_signal_recipients": null,
        "notify_only_on_change": false,
        "recurrence_json": null,
        "one_time": false,
        "misfire_policy": "SKIP_TO_NEXT",
        "safe_readonly_mode": true,
        "sync_to_google_docs": false,
        "sync_to_google_calendar": false,
        "is_active": true
    });
    let (status, schedule) = call(&ctx, Method::POST, "/api/schedules", Some(routine)).await;
    assert_eq!(status, StatusCode::OK);
    let id = schedule["id"].as_str().expect("id").to_string();
    let (_, sources) = call(
        &ctx,
        Method::GET,
        &format!("/api/io/inbound?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(sources["sources"].as_array().unwrap().len(), 1);

    let (status, _) = call(&ctx, Method::DELETE, &format!("/api/schedules/{id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, sources) = call(
        &ctx,
        Method::GET,
        &format!("/api/io/inbound?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert!(sources["sources"].as_array().unwrap().is_empty());
}
