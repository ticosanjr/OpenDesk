use super::*;

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
async fn task_fork_and_delete_with_queued_job() {
    let ctx = test_support::context().await;
    let (_, task) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?workspace_id={}", ctx.workspace_id),
        Some(json!({ "title": "Fork", "description": "teste" })),
    )
    .await;
    let task_id = task["id"].as_str().expect("task").to_string();
    let (status, forked) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{task_id}/fork"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(forked["id"], task_id);

    let job = ctx
        .state
        .queue
        .enqueue(queue_db::EnqueueInput {
            workspace_id: ctx.workspace_id.clone(),
            task_id: Some(task_id.clone()),
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
            output_dir: None,
            priority: 2,
            max_retries: 1,
        })
        .await
        .expect("job");
    let (status, _) = call(&ctx, Method::DELETE, &format!("/api/tasks/{task_id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let stored = queue_db::get(&ctx.state.db, &job.id)
        .await
        .expect("job row");
    assert_eq!(stored.status, "CANCELLED");
}

#[tokio::test]
async fn task_continuation_transition_and_diagnostics_cover_follow_up_path() {
    let ctx = test_support::context().await;
    let (_, task) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?workspace_id={}", ctx.workspace_id),
        Some(json!({ "title": "Follow-up", "description": "teste" })),
    )
    .await;
    let id = task["id"].as_str().expect("task").to_string();

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{id}/messages"),
        Some(json!({
            "content": "continue daqui",
            "continue_execution": true,
            "model": "opencode-go/glm-5.3"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let (_, messages) = call(
        &ctx,
        Method::GET,
        &format!("/api/tasks/{id}/messages"),
        None,
    )
    .await;
    assert_eq!(messages.as_array().expect("messages").len(), 1);

    let (status, cancelled) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{id}/transition"),
        Some(json!({ "status": "CANCELLED" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(cancelled["task"]["status"], "CANCELLED");

    runs::create(
        &ctx.state.db,
        runs::CreateRunInput {
            id: "run-diag".into(),
            queue_job_id: None,
            workspace_id: ctx.workspace_id.clone(),
            task_id: Some(id.clone()),
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
    let (status, diagnostics) = call(
        &ctx,
        Method::GET,
        &format!("/api/tasks/{id}/diagnostics"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(diagnostics["runs"].as_array().expect("runs").len(), 1);
}

#[tokio::test]
async fn task_workflow_hold_and_optional_title() {
    let ctx = test_support::context().await;
    let (status, created) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?workspace_id={}", ctx.workspace_id),
        Some(json!({
            "title": "",
            "description": "Primeira linha\nRestante da descrição"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(created["title"], "Primeira linha");
    assert_eq!(created["workflow_state"], "READY");
    let id = created["id"].as_str().expect("id").to_string();

    let (status, held) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{id}/hold"),
        Some(json!({ "held": true })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{held}");
    assert_eq!(held["workflow_state"], "HELD");
    assert_eq!(held["status"], "TODO");

    let (status, resumed) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{id}/hold"),
        Some(json!({ "held": false })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resumed}");
    assert_eq!(resumed["workflow_state"], "READY");

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{id}/transition"),
        Some(json!({ "status": "IN_PROGRESS" })),
    )
    .await;
    assert!(
        !status.is_success(),
        "engine offline must reject fake running"
    );

    let (_, task) = call(&ctx, Method::GET, &format!("/api/tasks/{id}"), None).await;
    assert_eq!(task["status"], "TODO");
    assert_eq!(task["workflow_state"], "READY");
}

#[tokio::test]
async fn ask_user_interaction_blocks_and_resumes_the_task() {
    let ctx = test_support::context().await;
    let (_, created) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?workspace_id={}", ctx.workspace_id),
        Some(json!({
            "description": "Tarefa que precisa de esclarecimento",
            "tags": []
        })),
    )
    .await;
    let id = created["id"].as_str().expect("id").to_string();

    let (status, interaction) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks/{id}/interactions"),
        Some(json!({
            "question": "Qual formato você quer?",
            "options": ["PDF", "Markdown"],
            "allow_custom": true,
            "timeout_minutes": 5
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{interaction}");
    assert_eq!(interaction["status"], "PENDING");
    let interaction_id = interaction["id"]
        .as_str()
        .expect("interaction id")
        .to_string();

    let (_, task) = call(&ctx, Method::GET, &format!("/api/tasks/{id}"), None).await;
    assert_eq!(task["status"], "IN_REVIEW");
    assert_eq!(task["phase_status"], "awaiting_input");

    let (status, resolved) = call(
        &ctx,
        Method::POST,
        &format!("/api/interactions/{interaction_id}/resolve"),
        Some(json!({ "selected": ["Markdown"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{resolved}");
    assert_eq!(resolved["status"], "RESOLVED");

    let (_, pending) = call(
        &ctx,
        Method::GET,
        &format!("/api/tasks/{id}/interactions"),
        None,
    )
    .await;
    assert!(pending.as_array().expect("pending").is_empty());

    let (_, task) = call(&ctx, Method::GET, &format!("/api/tasks/{id}"), None).await;
    assert_eq!(task["status"], "IN_PROGRESS");
    assert_eq!(task["phase_status"], "running");
}

#[tokio::test]
async fn missed_runs_can_be_listed_and_ignored() {
    let ctx = test_support::context().await;
    let input: crate::db::schedules::CreateScheduleInput = serde_json::from_value(json!({
        "workspace_id": ctx.workspace_id,
        "name": "Rotina perdida",
        "command_or_prompt": "Resuma",
        "cron_expression": "0 8 * * *",
        "timezone": "UTC",
        "trigger_type": "CRON",
        "target_kind": "PROMPT"
    }))
    .expect("schedule input");
    let schedule = crate::db::schedules::create(&ctx.state.db, input)
        .await
        .expect("schedule");
    let missed = crate::db::missed_runs::create_or_ignore(
        &ctx.state.db,
        &ctx.workspace_id,
        &schedule.id,
        "2026-01-01T08:00:00Z",
        "OFFLINE",
    )
    .await
    .expect("missed");

    let (status, list) = call(
        &ctx,
        Method::GET,
        &format!("/api/missed-runs?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list.as_array().expect("list").len(), 1);

    let (status, ignored) = call(
        &ctx,
        Method::POST,
        &format!("/api/missed-runs/{}/ignore", missed.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{ignored}");
    assert_eq!(ignored["status"], "IGNORED");

    let (_, pending) = call(
        &ctx,
        Method::GET,
        &format!(
            "/api/missed-runs?workspace_id={}&status=PENDING",
            ctx.workspace_id
        ),
        None,
    )
    .await;
    assert!(pending.as_array().expect("pending").is_empty());
}
