use super::*;
use crate::db::{artifacts, deliveries, queue as queue_db, runs};
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
async fn retry_endpoint_resends_failed_delivery() {
    let ctx = test_support::context().await;
    let delivery = deliveries::create_pending(
        &ctx.state.db,
        &ctx.workspace_id,
        None,
        "GDOCS",
        "gdocs:api:retry",
    )
    .await
    .expect("create")
    .expect("created");
    deliveries::mark_failed_retryable(
        &ctx.state.db,
        &delivery.id,
        "timeout",
        &json!({ "artifact_id": "missing" }).to_string(),
        0,
    )
    .await
    .expect("schedule");

    let (status, value) = call(
        &ctx,
        Method::POST,
        &format!("/api/integrations/deliveries/{}/retry", delivery.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(value["status"], "SKIPPED");
    assert_eq!(value["last_error"], "artifact not found");
}

#[tokio::test]
async fn retry_endpoint_rejects_successful_deliveries() {
    let ctx = test_support::context().await;
    let delivery = deliveries::create_pending(
        &ctx.state.db,
        &ctx.workspace_id,
        None,
        "DRIVE",
        "drive:api:done",
    )
    .await
    .expect("create")
    .expect("created");
    deliveries::mark_success(
        &ctx.state.db,
        &delivery.id,
        None,
        Some("https://example.test"),
    )
    .await
    .expect("success");

    let (status, value) = call(
        &ctx,
        Method::POST,
        &format!("/api/integrations/deliveries/{}/retry", delivery.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        value["message"]
            .as_str()
            .is_some_and(|message| message.contains("FAILED"))
    );
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

#[tokio::test]
async fn task_lifecycle_covers_crud_memory_and_messages() {
    let ctx = test_support::context().await;
    let (status, created) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?workspace_id={}", ctx.workspace_id),
        Some(json!({
            "title": "Relatório Trimestral",
            "description": "Resumo executivo",
            "priority": 3,
            "assigned_agent": "build",
            "preferred_model": "opencode-go/glm-5.3",
            "effort_level": "high",
            "notify_email": "qa@example.com",
            "export_to_gdocs": true,
            "tags": ["qa", "trimestral"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = created["id"].as_str().expect("task id").to_string();
    assert_eq!(created["preferred_model"], "opencode-go/glm-5.3");

    let (status, list) = call(&ctx, Method::GET, "/api/tasks", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().expect("tasks").len(), 1);

    let (status, fetched) = call(&ctx, Method::GET, &format!("/api/tasks/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["title"], "Relatório Trimestral");

    let (status, updated) = call(
        &ctx,
        Method::PATCH,
        &format!("/api/tasks/{id}"),
        Some(json!({
            "title": "Relatório Trimestral v2",
            "priority": 4,
            "clear_model": true,
            "tags": ["qa"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["title"], "Relatório Trimestral v2");
    assert!(updated["preferred_model"].is_null());

    let (status, moved) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{id}/move"),
        Some(json!({ "status": "IN_PROGRESS" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(moved["status"], "IN_PROGRESS");

    let (status, transitioned) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{id}/transition"),
        Some(json!({ "status": "DONE" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(transitioned["task"]["status"], "DONE");

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{id}/transition"),
        Some(json!({ "status": "NOPE" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, diagnostics) = call(
        &ctx,
        Method::GET,
        &format!("/api/tasks/{id}/diagnostics"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(diagnostics.is_object());

    let (status, memory) = call(&ctx, Method::GET, &format!("/api/tasks/{id}/memory"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(memory["content"], "");

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{id}/memory"),
        Some(json!({ "content": "preferir resumo curto" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        std::fs::read_to_string(ctx.workspace_root.join(".opencode/MEMORY.md"))
            .expect("memory")
            .contains("preferir resumo curto")
    );

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{id}/messages"),
        Some(json!({ "content": "comentário", "continue_execution": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, messages) = call(
        &ctx,
        Method::GET,
        &format!("/api/tasks/{id}/messages"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(messages.as_array().expect("messages").len(), 1);

    let (status, _) = call(&ctx, Method::DELETE, &format!("/api/tasks/{id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&ctx, Method::GET, &format!("/api/tasks/{id}"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn task_dispatch_and_plan_validation_are_guarded() {
    let ctx = test_support::context().await;
    let (_, created) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?workspace_id={}", ctx.workspace_id),
        Some(json!({ "title": "Dispatch", "description": "teste" })),
    )
    .await;
    let id = created["id"].as_str().expect("id");

    let (status, body) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{id}/dispatch"),
        Some(json!({ "manual_model": "sem-provider" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["message"]
            .as_str()
            .unwrap_or_default()
            .contains("modelo inválido")
    );

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{id}/approve-plan"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{id}/dispatch"),
        Some(json!({ "manual_model": "opencode-go/glm-5.3" })),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn schedule_lifecycle_preview_toggle_and_runs() {
    let ctx = test_support::context().await;
    let (status, created) = call(
        &ctx,
        Method::POST,
        "/api/schedules",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "name": "Rotina Trimestral",
            "description": "fechamento",
            "cron_expression": "0 9 * * 1",
            "timezone": "UTC",
            "command_or_prompt": "Gerar resumo trimestral",
            "trigger_type": "CRON",
            "target_kind": "PROMPT"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = created["id"].as_str().expect("schedule id").to_string();
    assert!(created["next_run_at"].is_string());

    let (status, list) = call(&ctx, Method::GET, "/api/schedules", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().expect("schedules").len(), 1);

    let (status, fetched) = call(&ctx, Method::GET, &format!("/api/schedules/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["name"], "Rotina Trimestral");

    let (status, updated) = call(
        &ctx,
        Method::PATCH,
        &format!("/api/schedules/{id}"),
        Some(json!({ "description": "fechamento mensal", "priority": null })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["description"], "fechamento mensal");

    let (status, toggled) = call(
        &ctx,
        Method::POST,
        &format!("/api/schedules/{id}/active"),
        Some(json!({ "active": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(toggled["is_active"], false);

    let (status, preview) = call(
        &ctx,
        Method::POST,
        "/api/schedules/preview",
        Some(json!({ "recurrence": { "frequency": "daily", "time": "08:00" }, "timezone": "UTC" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        preview["next_runs"]
            .as_array()
            .is_some_and(|runs| !runs.is_empty())
    );
    assert!(preview["cron_expression"].is_string());

    let (status, runs) = call(
        &ctx,
        Method::GET,
        &format!("/api/schedules/{id}/runs"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(runs.is_array());

    let (status, _dry) = call(
        &ctx,
        Method::POST,
        &format!("/api/schedules/{id}/dry-run"),
        Some(json!({ "target_kind": "PROMPT" })),
    )
    .await;
    assert!(matches!(
        status,
        StatusCode::OK | StatusCode::SERVICE_UNAVAILABLE
    ));

    let (status, _) = call(&ctx, Method::DELETE, &format!("/api/schedules/{id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn studio_ops_manage_assets_duplicates_and_backups() {
    let ctx = test_support::context().await;
    let query = format!("scope=workspace&workspace_id={}", ctx.workspace_id);
    for (id, kind) in [
        ("qa-agent", "agent"),
        ("qa-skill", "skill"),
        ("qa-command", "command"),
    ] {
        let uri = match kind {
            "agent" => format!("/api/studio/agent?id={id}&{query}"),
            "skill" => format!("/api/studio/skill?id={id}&{query}"),
            _ => format!("/api/studio/command?id={id}&{query}"),
        };
        let body = match kind {
            "agent" => json!({
                "name": "QA Agent",
                "description": "Audita entregas",
                "mode": "subagent",
                "system": "Você audita."
            }),
            "skill" => json!({
                "name": "QA Skill",
                "description": "Checklist de QA",
                "body": "## Processo\n1. Verifique."
            }),
            _ => json!({
                "description": "Executa QA",
                "agent": "qa-agent",
                "template": "Revise $ARGUMENTS"
            }),
        };
        let (status, _) = call(&ctx, Method::PUT, &uri, Some(body)).await;
        assert_eq!(status, StatusCode::OK, "{kind} write");
    }

    for uri in [
        format!("/api/studio/effective?{query}"),
        "/api/studio/catalog".to_string(),
        "/api/studio/runtime".to_string(),
    ] {
        let (status, body) = call(&ctx, Method::GET, &uri, None).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert!(!body.is_null(), "{uri}");
    }

    let (status, duplicated) = call(
        &ctx,
        Method::POST,
        &format!("/api/studio/agent/qa-agent/duplicate?{query}"),
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let copy_id = duplicated["id"].as_str().expect("copy id").to_string();

    let (status, _toggled) = call(
        &ctx,
        Method::POST,
        &format!("/api/studio/agent/{copy_id}/toggle?{query}"),
        Some(json!({ "enabled": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, backups) = call(
        &ctx,
        Method::GET,
        &format!("/api/studio/agent/qa-agent/backups?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let backup_id = backups["backups"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item.get("id").or_else(|| item.get("backup_id")))
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(backup_id) = backup_id {
        let (status, _) = call(
            &ctx,
            Method::POST,
            &format!("/api/studio/agent/qa-agent/restore?{query}"),
            Some(json!({ "backup_id": backup_id })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    let (status, copied) = call(
        &ctx,
        Method::POST,
        &format!("/api/studio/skill/qa-skill/copy?{query}"),
        Some(json!({ "new_id": "qa-skill-copy" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{copied}");
    assert_eq!(copied["scope"], "workspace");
    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/studio/skill/qa-skill-copy?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/studio/agent/{copy_id}?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn file_endpoints_read_tree_content_raw_and_search() {
    let ctx = test_support::context().await;
    let notes = ctx.workspace_root.join("notes");
    std::fs::create_dir_all(&notes).expect("notes dir");
    std::fs::write(notes.join("report.md"), "# Relatório\nconteúdo").expect("report");
    std::fs::create_dir_all(ctx.workspace_root.join("nested/deep")).expect("nested");
    std::fs::write(ctx.workspace_root.join("nested/deep/data.txt"), "dados").expect("data");
    let base = "/api/files";

    let (status, tree) = call(
        &ctx,
        Method::GET,
        &format!("{base}/tree?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        tree.as_array()
            .is_some_and(|items| items.iter().any(|item| item["name"] == "notes"))
    );

    let (status, content) = call(
        &ctx,
        Method::GET,
        &format!(
            "{base}/content?workspace_id={}&path=notes/report.md",
            ctx.workspace_id
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        content["content"]
            .as_str()
            .unwrap_or_default()
            .contains("Relatório")
    );

    let (status, raw) = call(
        &ctx,
        Method::GET,
        &format!(
            "{base}/raw?workspace_id={}&path=notes/report.md",
            ctx.workspace_id
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(raw.as_str().unwrap_or_default().contains("Relatório"));

    let (status, found) = call(
        &ctx,
        Method::GET,
        &format!("{base}/search?workspace_id={}&q=repo", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        found
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["name"] == "report.md"))
    );

    let (status, _) = call(
        &ctx,
        Method::GET,
        &format!(
            "{base}/content?workspace_id={}&path=../escape",
            ctx.workspace_id
        ),
        None,
    )
    .await;
    assert!(matches!(
        status,
        StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND
    ));
}

#[tokio::test]
async fn global_search_finds_tasks_schedules_comments_and_agents() {
    let ctx = test_support::context().await;
    let (_, task) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?workspace_id={}", ctx.workspace_id),
        Some(json!({ "title": "Análise Trimestral", "description": "dados" })),
    )
    .await;
    let task_id = task["id"].as_str().expect("task");
    let (_, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{task_id}/messages"),
        Some(json!({ "content": "observação trimestral", "continue_execution": false })),
    )
    .await;
    let (_, _) = call(
        &ctx,
        Method::POST,
        "/api/schedules",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "name": "Fechamento Trimestral",
            "command_or_prompt": "consolidar",
            "cron_expression": "0 9 * * 1",
            "timezone": "UTC"
        })),
    )
    .await;
    let query = format!("scope=workspace&workspace_id={}", ctx.workspace_id);
    let (_, _) = call(
        &ctx,
        Method::PUT,
        &format!("/api/studio/agent?id=auditor-trimestral&{query}"),
        Some(json!({
            "name": "Auditor Trimestral",
            "description": "audita",
            "mode": "subagent",
            "system": "Você audita."
        })),
    )
    .await;

    let (status, hits) = call(
        &ctx,
        Method::GET,
        &format!("/api/search?q=trimestral&workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let kinds = hits
        .as_array()
        .expect("hits")
        .iter()
        .filter_map(|hit| hit["kind"].as_str())
        .collect::<Vec<_>>();
    assert!(kinds.contains(&"task"));
    assert!(kinds.contains(&"schedule"));
    assert!(kinds.contains(&"comment"));
    assert!(kinds.contains(&"agent"));
}

#[tokio::test]
async fn queue_budget_cost_and_chat_endpoints_work_offline() {
    let ctx = test_support::context().await;

    let (status, queue) = call(&ctx, Method::GET, "/api/queue", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(queue.is_array());
    let (status, status_body) = call(&ctx, Method::GET, "/api/queue/status", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(status_body.is_object());
    let (status, _) = call(&ctx, Method::POST, "/api/queue/pause", None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&ctx, Method::POST, "/api/queue/resume", None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, concurrency) = call(
        &ctx,
        Method::PUT,
        "/api/queue/concurrency",
        Some(json!({ "concurrency": 3 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(concurrency.is_object());

    let (status, budget) = call(
        &ctx,
        Method::PUT,
        "/api/budget",
        Some(json!({ "daily_limit_usd": 12.5, "monthly_limit_usd": 200.0 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(budget["daily_limit_usd"], 12.5);
    let (status, budget_status) = call(&ctx, Method::GET, "/api/budget/status", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(budget_status["daily_pct"].is_number());
    let (status, _mix) = call(&ctx, Method::GET, "/api/cost/mix", None).await;
    assert_eq!(status, StatusCode::OK);

    let (status, thread) = call(
        &ctx,
        Method::POST,
        "/api/chat/threads",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "title": "Conversa QA",
            "agent": "build",
            "model": "opencode-go/glm-5.3",
            "source": "desktop"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let thread_id = thread["id"].as_str().expect("thread").to_string();
    let (status, list) = call(
        &ctx,
        Method::GET,
        &format!("/api/chat/threads?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().expect("threads").len(), 1);
    let (status, renamed) = call(
        &ctx,
        Method::PATCH,
        &format!("/api/chat/threads/{thread_id}"),
        Some(json!({ "title": "Conversa QA 2" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(renamed["title"], "Conversa QA 2");
    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/chat/threads/{thread_id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn router_rules_and_settings_round_trip() {
    let ctx = test_support::context().await;
    let query = format!("?workspace_id={}", ctx.workspace_id);

    let (status, config) = call(
        &ctx,
        Method::GET,
        &format!("/api/router/config{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(config["workspace_id"], ctx.workspace_id);

    let (status, _) = call(
        &ctx,
        Method::PUT,
        "/api/router/model",
        Some(json!({ "model": "opencode-go/glm-5.3" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &ctx,
        Method::PUT,
        "/api/router/model",
        Some(json!({ "model": "invalido" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &ctx,
        Method::PUT,
        "/api/router/blend",
        Some(json!({ "blend": 0.4 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &ctx,
        Method::PUT,
        "/api/router/blend",
        Some(json!({ "blend": 2.0 })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, rule) = call(
        &ctx,
        Method::POST,
        "/api/router/rules",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "name": "QA barato",
            "condition_json": "{\"tags\":[\"qa\"]}",
            "target_mode": "build",
            "target_model": "opencode-go/glm-5.3",
            "target_effort": "low"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let rule_id = rule["id"].as_str().expect("rule id").to_string();
    let (status, rules) = call(
        &ctx,
        Method::GET,
        &format!("/api/router/rules{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rules.as_array().expect("rules").len(), 1);
    let (status, updated) = call(
        &ctx,
        Method::PATCH,
        &format!("/api/router/rules/{rule_id}"),
        Some(json!({ "name": "QA barato v2", "is_enabled": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["name"], "QA barato v2");
    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/router/rules/{rule_id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn queue_jobs_can_be_prioritized_cancelled_and_retried() {
    let ctx = test_support::context().await;
    let job = ctx
        .state
        .queue
        .enqueue(queue_db::EnqueueInput {
            workspace_id: ctx.workspace_id.clone(),
            task_id: None,
            schedule_id: None,
            session_id: None,
            concurrency_group: None,
            prompt_payload: "teste".into(),
            resolved_mode: "build".into(),
            resolved_agent: "build".into(),
            resolved_model: "opencode-go/glm-5.3".into(),
            resolved_effort: "low".into(),
            job_kind: "task".into(),
            scheduled_for: None,
            output_dir: Some(ctx.workspace_root.to_string_lossy().to_string()),
            priority: 2,
            max_retries: 1,
        })
        .await
        .expect("enqueue");
    let id = job.id;

    let (status, list) = call(&ctx, Method::GET, "/api/queue", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().expect("queue").len(), 1);

    let (status, prioritized) = call(
        &ctx,
        Method::PATCH,
        &format!("/api/queue/{id}/priority"),
        Some(json!({ "priority": 4 })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(prioritized["priority"], 4);

    let (status, cancelled) =
        call(&ctx, Method::POST, &format!("/api/queue/{id}/cancel"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cancelled["status"], "CANCELLED");

    let (status, retried) = call(&ctx, Method::POST, &format!("/api/queue/{id}/retry"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retried["status"], "QUEUED");
}

#[tokio::test]
async fn io_bus_sources_can_be_managed_and_tools_listed() {
    let ctx = test_support::context().await;
    let query = format!("?workspace_id={}", ctx.workspace_id);

    let (status, inbound) = call(
        &ctx,
        Method::POST,
        &format!("/api/io/inbound{query}"),
        Some(json!({
            "name": "Webhook QA",
            "source_type": "WEBHOOK_HTTP",
            "config": {},
            "auto_execute": false,
            "is_enabled": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let inbound_id = inbound["id"].as_str().expect("inbound").to_string();
    let (status, list) = call(&ctx, Method::GET, &format!("/api/io/inbound{query}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(list["sources"].is_array());
    let (status, updated) = call(
        &ctx,
        Method::PATCH,
        &format!("/api/io/inbound/{inbound_id}"),
        Some(json!({
            "name": "Webhook QA 2",
            "source_type": "WEBHOOK_HTTP",
            "config": {},
            "auto_execute": false,
            "is_enabled": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["name"], "Webhook QA 2");

    let (status, outbound) = call(
        &ctx,
        Method::POST,
        &format!("/api/io/outbound{query}"),
        Some(json!({
            "name": "Pasta QA",
            "sink_type": "ATOMIC_FS",
            "config": { "path": "output" },
            "is_enabled": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let outbound_id = outbound["id"].as_str().expect("outbound").to_string();
    let (status, sinks) = call(&ctx, Method::GET, &format!("/api/io/outbound{query}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(sinks["sinks"].is_array());

    let (status, tools) = call(&ctx, Method::GET, "/api/io/tools", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(tools.is_array());

    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/io/inbound/{inbound_id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/io/outbound/{outbound_id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn proposals_can_be_created_rejected_and_failed_on_apply() {
    let ctx = test_support::context().await;
    let query = format!("?workspace_id={}", ctx.workspace_id);

    let (status, proposal) = call(
        &ctx,
        Method::POST,
        &format!("/api/proposals{query}"),
        Some(json!({
            "kind": "AGENT",
            "action": "CREATE",
            "title": "Agente QA",
            "payload": {}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = proposal["id"].as_str().expect("proposal").to_string();
    let (status, listed) = call(&ctx, Method::GET, &format!("/api/proposals{query}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(listed["proposals"].as_array().expect("proposals").len(), 1);

    let (status, rejected) = call(
        &ctx,
        Method::POST,
        &format!("/api/proposals/{id}/resolve"),
        Some(json!({ "approved": false, "resolved_by": "qa" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rejected["status"], "REJECTED");

    let (status, broken) = call(
        &ctx,
        Method::POST,
        &format!("/api/proposals{query}"),
        Some(json!({
            "kind": "AGENT",
            "action": "CREATE",
            "title": "Agente quebrado",
            "payload": {}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let broken_id = broken["id"].as_str().expect("broken").to_string();
    let (status, failed) = call(
        &ctx,
        Method::POST,
        &format!("/api/proposals/{broken_id}/resolve"),
        Some(json!({ "approved": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(failed["status"], "FAILED");
}

#[tokio::test]
async fn artifacts_api_lists_pins_and_serves_content() {
    let ctx = test_support::context().await;
    let out = ctx.workspace_root.join("output/qa");
    std::fs::create_dir_all(&out).expect("out");
    let file = out.join("report.md");
    std::fs::write(&file, "# Artefato\nconteúdo").expect("artifact");
    runs::create(
        &ctx.state.db,
        runs::CreateRunInput {
            id: "run-qa".into(),
            queue_job_id: None,
            workspace_id: ctx.workspace_id.clone(),
            task_id: None,
            schedule_id: None,
            parent_run_id: None,
            phase: Some("build".into()),
            agent_name: "build".into(),
            execution_mode: "BUILD_DIRECT".into(),
            provider_id: Some("opencode-go".into()),
            model_id: "glm-5.3".into(),
            effort_level: "low".into(),
            pricing_snapshot_json: None,
            log_path: None,
        },
    )
    .await
    .expect("run");
    let artifact = artifacts::create(
        &ctx.state.db,
        artifacts::CreateArtifactInput {
            workspace_id: ctx.workspace_id.clone(),
            task_id: None,
            run_id: "run-qa".into(),
            file_path: "output/qa/report.md".into(),
            absolute_path: file.to_string_lossy().to_string(),
            file_type: "md".into(),
            file_size_bytes: 20,
            content_sha256: None,
            previous_sha256: None,
            action_type: "CREATED".into(),
            created_by_agent: "build".into(),
            creation_reason: "QA".into(),
            diff_patch: None,
        },
    )
    .await
    .expect("artifact");
    let id = artifact.id;

    let (status, list) = call(
        &ctx,
        Method::GET,
        &format!("/api/artifacts?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().expect("artifacts").len(), 1);
    let (status, fetched) = call(&ctx, Method::GET, &format!("/api/artifacts/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["file_path"], "output/qa/report.md");
    let (status, content) = call(
        &ctx,
        Method::GET,
        &format!("/api/artifacts/{id}/content"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        content["content"]
            .as_str()
            .unwrap_or_default()
            .contains("Artefato")
    );

    let (status, pinned) = call(
        &ctx,
        Method::POST,
        &format!("/api/artifacts/{id}/pin"),
        Some(json!({ "pinned": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(pinned["pinned"], true);
    let (status, pinned_list) = call(
        &ctx,
        Method::GET,
        &format!("/api/artifacts/pinned?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(pinned_list.as_array().expect("pinned").len(), 1);
}

#[tokio::test]
async fn engine_permissions_and_diagnostics_are_available_offline() {
    let ctx = test_support::context().await;
    for uri in [
        "/api/engine/status",
        "/api/engine/diagnostics",
        "/api/models/health",
        "/api/permissions",
        "/api/agents/states",
    ] {
        let (status, body) = call(&ctx, Method::GET, uri, None).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert!(!body.is_null(), "{uri}");
    }
    let (status, _) = call(&ctx, Method::POST, "/api/engine/detect", None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/models/health/clear",
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn smtp_and_signal_settings_round_trip() {
    let ctx = test_support::context().await;
    let query = format!("?workspace_id={}", ctx.workspace_id);

    let (status, empty) = call(
        &ctx,
        Method::GET,
        &format!("/api/integrations/smtp{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(empty["configured"], false);

    let (status, saved) = call(
        &ctx,
        Method::PUT,
        "/api/integrations/smtp",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "host": "smtp.example.com",
            "port": 587,
            "username": "qa",
            "from_email": "qa@example.com",
            "use_tls": true,
            "password": "secret"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["configured"], true);
    assert_eq!(saved["password_set"], true);

    let (status, _) = call(
        &ctx,
        Method::PUT,
        "/api/integrations/smtp",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "host": "",
            "from_email": "qa@example.com"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &ctx,
        Method::PUT,
        "/api/integrations/smtp",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "host": "smtp.example.com",
            "from_email": "sem-arroba"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, signal) = call(&ctx, Method::GET, "/api/integrations/signal", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(signal["config"].is_object());
    let (status, updated) = call(
        &ctx,
        Method::PUT,
        "/api/integrations/signal",
        Some(json!({
            "enabled": false,
            "account": "+5511999999999",
            "cli_path": "",
            "allowlist": [" +5511888888888 "],
            "workspace_id": ctx.workspace_id,
            "agent": ""
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["config"]["account"], "+5511999999999");
    assert_eq!(updated["config"]["allowlist"][0], "+5511888888888");
}

#[tokio::test]
async fn recipes_sync_and_dry_run_flow_are_recorded() {
    let ctx = test_support::context().await;
    let recipes_dir = ctx.workspace_root.join(".opencode/recipes");
    std::fs::create_dir_all(&recipes_dir).expect("recipes dir");
    std::fs::write(
        recipes_dir.join("qa-flow.yaml"),
        r#"id: qa-flow
name: Fluxo QA
description: Teste de fluxo
enabled: true
triggers: []
flow:
  - type: step
    id: passo-1
    name: Primeiro passo
    mode: build
    agent: build
    effort: low
    prompt: "Gere um rascunho em {{run.output_dir}}/rascunho.md"
"#,
    )
    .expect("recipe");

    let (status, synced) = call(&ctx, Method::POST, "/api/recipes/sync", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(!synced.is_null());
    let (status, listed) = call(
        &ctx,
        Method::GET,
        &format!("/api/recipes?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        listed["recipes"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["id"] == "qa-flow"))
    );

    let (status, recipe) = call(&ctx, Method::GET, "/api/recipes/qa-flow", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(recipe["recipe"]["name"], "Fluxo QA");
    assert!(
        recipe["yaml"]
            .as_str()
            .unwrap_or_default()
            .contains("Fluxo QA")
    );

    let (status, run) = call(
        &ctx,
        Method::POST,
        "/api/recipes/qa-flow/run",
        Some(json!({ "dry_run": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let run_id = run["id"].as_str().expect("run").to_string();
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;

    let (status, runs) = call(
        &ctx,
        Method::GET,
        &format!("/api/recipes/runs?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        runs["runs"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["id"] == run_id))
    );
    let (status, detail) = call(
        &ctx,
        Method::GET,
        &format!("/api/recipes/runs/{run_id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(detail["run"].is_object());
    let (status, hitl) = call(
        &ctx,
        Method::GET,
        &format!("/api/recipes/hitl?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(hitl["requests"].is_array());
}
