use super::*;

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
async fn queue_budget_cost_and_chat_endpoints_work_offline() {
    let ctx = test_support::context().await;

    let (status, queue) = call(&ctx, Method::GET, "/api/queue", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(queue.is_array());
    let (status, status_body) = call(&ctx, Method::GET, "/api/queue/status", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(status_body.is_object());
    assert!(status_body["max_concurrency"].is_number());
    assert!(status_body["max_queue_size"].is_number());
    assert!(status_body["group_usage"].is_array());
    let (status, policy) = call(&ctx, Method::GET, "/api/queue/config", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(policy["default_agent_concurrency"], 2);
    let (status, saved) = call(
        &ctx,
        Method::PUT,
        "/api/queue/config",
        Some(json!({
            "max_concurrency": 5,
            "max_queue_size": 50,
            "default_agent_concurrency": 3,
            "group_limits": { "default": 5, "light_api": 4 }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["max_concurrency"], 5);
    assert_eq!(saved["group_limits"]["light_api"], 4);
    let (status, _) = call(
        &ctx,
        Method::PUT,
        "/api/queue/config",
        Some(json!({
            "max_concurrency": 0,
            "max_queue_size": 50,
            "default_agent_concurrency": 3,
            "group_limits": {}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
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
            resolved_harness: "opencode".into(),
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

#[tokio::test]
async fn meta_builder_creates_skill_agent_and_command() {
    let ctx = test_support::context().await;
    let query = format!("?workspace_id={}", ctx.workspace_id);
    for (prompt, kind, id) in [
        (
            "crie uma skill chamada meta-skill focada em auditoria",
            "skill",
            "meta-skill",
        ),
        (
            "crie um subagente chamado meta-agent focado em revisão",
            "agent",
            "meta-agent",
        ),
        (
            "crie um comando /meta-cmd para auditar entregas",
            "command",
            "meta-cmd",
        ),
    ] {
        let (status, result) = call(
            &ctx,
            Method::POST,
            &format!("/api/meta/build{query}"),
            Some(json!({ "prompt": prompt })),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{prompt}: {result}");
        assert_eq!(result["kind"], kind);
        assert_eq!(result["id"], id);
        assert!(result["path"].as_str().is_some_and(|path| !path.is_empty()));
    }

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/meta/build{query}"),
        Some(json!({ "prompt": "nada reconhecível" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn blueprints_list_apply_and_skip_existing_files() {
    let ctx = test_support::context().await;
    let query = format!("workspace_id={}", ctx.workspace_id);
    let (status, list) = call(&ctx, Method::GET, "/api/blueprints", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().expect("blueprints").len(), 4);

    let (status, applied) = call(
        &ctx,
        Method::POST,
        &format!("/api/blueprints/conteudo/apply?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{applied}");
    assert!(!applied["created"].as_array().expect("created").is_empty());
    assert!(
        ctx.workspace_root
            .join(".opencode/agents/pesquisador.md")
            .is_file()
    );

    let (status, again) = call(
        &ctx,
        Method::POST,
        &format!("/api/blueprints/conteudo/apply?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!again["skipped"].as_array().expect("skipped").is_empty());

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/blueprints/missing/apply?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn dashboard_decodes_zero_cost_runs_as_real() {
    let ctx = test_support::context().await;
    crate::db::runs::create(
        &ctx.state.db,
        crate::db::runs::CreateRunInput {
            id: "run-zero-cost".into(),
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
    crate::db::runs::finish(
        &ctx.state.db,
        "run-zero-cost",
        "SUCCESS",
        &crate::db::runs::RunUsage::default(),
        10,
        Some("ok"),
        None,
    )
    .await
    .expect("finish");

    let (status, stats) = call(
        &ctx,
        Method::GET,
        &format!("/api/dashboard?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{stats}");
    assert_eq!(stats["runs_total"], 1);
    assert_eq!(stats["estimated_cost_usd"], 0.0);
    assert_eq!(stats["daily_cost_usd"], 0.0);
}

#[tokio::test]
async fn dashboard_counts_active_routines_and_enabled_recipes() {
    let ctx = test_support::context().await;
    let (status, schedule) = call(
        &ctx,
        Method::POST,
        "/api/schedules",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "name": "Rotina dashboard",
            "command_or_prompt": "Resuma",
            "cron_expression": "0 8 * * *",
            "timezone": "UTC"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{schedule}");
    let routine_id = schedule["id"].as_str().expect("routine id");
    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/schedules/{routine_id}/active"),
        Some(json!({ "active": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let yaml = "id: dashboard-recipe\nname: Receita dashboard\nenabled: false\ntriggers: []\nflow:\n  - type: step\n    id: passo-1\n    name: Passo\n    prompt: oi\n";
    let (status, _) = call(
        &ctx,
        Method::PUT,
        &format!(
            "/api/recipes/dashboard-recipe?workspace_id={}",
            ctx.workspace_id
        ),
        Some(json!({ "yaml": yaml })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!(
            "/api/recipes/dashboard-recipe/enabled?workspace_id={}",
            ctx.workspace_id
        ),
        Some(json!({ "enabled": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, stats) = call(
        &ctx,
        Method::GET,
        &format!("/api/dashboard?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{stats}");
    assert_eq!(stats["active_routines"], 1);
    assert_eq!(stats["enabled_recipes"], 1);
}

#[tokio::test]
async fn router_test_uses_opendesk_models_instead_of_the_opencode_default() {
    let ctx = test_support::context().await;
    std::fs::write(
        ctx.workspace_root.join("opencode.jsonc"),
        r#"{ "model": "other/provider-that-must-be-ignored" }"#,
    )
    .expect("config");

    let (status, saved) = call(
        &ctx,
        Method::PUT,
        "/api/router/models",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "default_model": "deepseek/deepseek-flash",
            "manual_model": "deepseek/deepseek-v4-pro"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["default_model"], "deepseek/deepseek-flash");
    assert_eq!(saved["manual_model"], "deepseek/deepseek-v4-pro");

    let (status, body) = call(
        &ctx,
        Method::POST,
        "/api/router/test",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "title": "Relatório executivo",
            "description": "Analise os dados e produza um relatório.",
            "tags": ["relatorio"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["default_model"], "deepseek/deepseek-flash");
    assert_eq!(body["manual_model"], "deepseek/deepseek-v4-pro");
    assert_eq!(body["decision"]["origin"], "COST_BENEFIT");
    assert_eq!(body["decision"]["selected_agent"], "build");
    assert!(
        body["decision"]["selected_model"]
            .as_str()
            .is_some_and(|value| value.starts_with("deepseek/deepseek-flash"))
    );

    let (status, manual) = call(
        &ctx,
        Method::POST,
        "/api/router/test",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "policy": "MANUAL",
            "title": "Teste manual",
            "description": "Use o modelo manual do workspace."
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{manual}");
    assert_eq!(manual["decision"]["origin"], "MANUAL");
    assert_eq!(
        manual["decision"]["selected_model"],
        "deepseek/deepseek-v4-pro"
    );

    let (status, _) = call(
        &ctx,
        Method::PUT,
        "/api/router/models",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "default_model": "sem-provider",
            "manual_model": null
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, cleared) = call(
        &ctx,
        Method::PUT,
        "/api/router/models",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "default_model": null,
            "manual_model": ""
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    assert!(cleared["default_model"].is_null());
    assert!(cleared["manual_model"].is_null());
}

#[tokio::test]
async fn model_test_reports_engine_offline_without_failing_the_api() {
    let ctx = test_support::context().await;
    let (status, body) = call(
        &ctx,
        Method::POST,
        "/api/models/test",
        Some(json!({ "model": "deepseek/deepseek-flash" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["ok"], false);
    assert_eq!(body["error_code"], "engine_offline");
}

#[tokio::test]
async fn updates_rollback_without_backup_is_actionable() {
    let ctx = test_support::context().await;
    let (status, body) = call(&ctx, Method::POST, "/api/updates/rollback", None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["message"]
            .as_str()
            .is_some_and(|value| value.contains("backup"))
    );
}

#[tokio::test]
async fn google_status_config_and_mcp_registration_work_offline() {
    let ctx = test_support::context().await;
    let query = format!("workspace_id={}", ctx.workspace_id);

    let (status, state) = call(
        &ctx,
        Method::GET,
        &format!("/api/integrations/google/status?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(state["connected"], false);

    let (status, config) = call(
        &ctx,
        Method::GET,
        &format!("/api/integrations/google/config?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        config["scopes"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );

    let (status, saved) = call(
        &ctx,
        Method::PUT,
        "/api/integrations/google/config",
        Some(json!({ "client_id": "client-id", "client_secret": "secret" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["client_id_set"], true);

    let (status, registered) = call(
        &ctx,
        Method::POST,
        "/api/integrations/google/mcp/register",
        Some(json!({ "workspace_id": ctx.workspace_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{registered}");
    let document = crate::config_sync::jsonc_editor::read_for_scope(
        &crate::config_sync::paths::StudioPaths::new(Some(ctx.workspace_root.clone())),
        "workspace",
    )
    .expect("config");
    assert!(
        crate::config_sync::jsonc_editor::list_mcp_servers(&document)
            .iter()
            .any(|server| server.name == "opendesk-google")
    );

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/integrations/google/disconnect",
        Some(json!({ "workspace_id": ctx.workspace_id })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn permissions_list_decide_agent_states_and_context() {
    let ctx = test_support::context().await;
    let query = format!("workspace_id={}", ctx.workspace_id);
    crate::db::permissions::create(
        &ctx.state.db,
        None,
        None,
        &ctx.workspace_id,
        "ses-1",
        "req-1",
        "edit",
        &["src/main.rs".into()],
    )
    .await
    .expect("permission");

    let (status, pending) = call(
        &ctx,
        Method::GET,
        &format!("/api/permissions?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(pending["requests"].as_array().expect("requests").len(), 1);

    let (status, decided) = call(
        &ctx,
        Method::POST,
        "/api/permissions/req-1/reply",
        Some(json!({ "decision": "once" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(decided["status"], "DECIDED");

    let (status, states) = call(
        &ctx,
        Method::GET,
        &format!("/api/agents/states?{query}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(states["agents"].is_array());

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/tasks?workspace_id=ws-test",
        Some(json!({
            "title": "Contexto",
            "description": "Sem execução",
            "execution_mode": "BUILD_DIRECT",
            "tags": []
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (_, tasks) = call(&ctx, Method::GET, "/api/tasks?workspace_id=ws-test", None).await;
    let task_id = tasks.as_array().expect("tasks")[0]["id"]
        .as_str()
        .expect("task id")
        .to_string();
    let (status, context) = call(
        &ctx,
        Method::GET,
        &format!("/api/tasks/{task_id}/context"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(context["percent"], 0.0);

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{task_id}/compact"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn workspace_email_recipient_can_be_cleared() {
    let ctx = test_support::context().await;
    let (status, created) = call(
        &ctx,
        Method::POST,
        "/api/workspaces",
        Some(json!({
            "name": "Email Clear",
            "root_path": ctx.temp.path().join("email-clear").to_string_lossy(),
            "default_email_recipient": "ops@example.com"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let id = created["id"].as_str().expect("id");

    let (status, updated) = call(
        &ctx,
        Method::PATCH,
        &format!("/api/workspaces/{id}"),
        Some(json!({ "default_email_recipient": null })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert!(updated["default_email_recipient"].is_null());

    let (_, reloaded) = call(&ctx, Method::GET, &format!("/api/workspaces/{id}"), None).await;
    assert!(reloaded["default_email_recipient"].is_null());
}
