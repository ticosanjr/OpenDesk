use super::*;

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
    let hits = hits.as_array().expect("hits");
    let task_link = hits
        .iter()
        .find(|hit| hit["kind"] == "task")
        .and_then(|hit| hit["link"].as_str())
        .expect("task link");
    assert_eq!(task_link, format!("/kanban?view=board&task={task_id}"));
    let comment_link = hits
        .iter()
        .find(|hit| hit["kind"] == "comment")
        .and_then(|hit| hit["link"].as_str())
        .expect("comment link");
    assert!(comment_link.starts_with("/kanban?view=board&task="));
    let schedule_link = hits
        .iter()
        .find(|hit| hit["kind"] == "schedule")
        .and_then(|hit| hit["link"].as_str())
        .expect("schedule link");
    assert!(schedule_link.starts_with("/routines?routine="));
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
async fn capture_creates_tasks_from_text_and_images() {
    let ctx = test_support::context().await;
    let (status, task) = call(
        &ctx,
        Method::POST,
        "/api/capture",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "content": "Comprar café\npara o escritório",
            "assigned_agent": "build"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(task["title"], "Comprar café");
    assert_eq!(task["agent_override"], true);
    assert!(
        task["tags_json"]
            .as_str()
            .unwrap_or_default()
            .contains("quick-capture")
    );

    let (status, image_task) = call(
        &ctx,
        Method::POST,
        "/api/capture",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "content": "",
            "filename": "print.png",
            "image_base64": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Y9Z0ZkAAAAASUVORK5CYII="
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        image_task["description"]
            .as_str()
            .unwrap_or_default()
            .contains("input/captures")
    );

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/capture",
        Some(json!({ "workspace_id": ctx.workspace_id, "content": "   " })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/capture",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "content": "x",
            "image_base64": "not-base64"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn runs_api_serves_logs_timeline_and_forks_from_run() {
    let ctx = test_support::context().await;
    let (_, task) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?workspace_id={}", ctx.workspace_id),
        Some(json!({ "title": "Run API", "description": "teste" })),
    )
    .await;
    let task_id = task["id"].as_str().expect("task").to_string();
    let log_path = ctx.workspace_root.join("run.log");
    std::fs::write(&log_path, "linha de log\n").expect("log");
    runs::create(
        &ctx.state.db,
        runs::CreateRunInput {
            id: "run-api".into(),
            queue_job_id: None,
            workspace_id: ctx.workspace_id.clone(),
            task_id: Some(task_id.clone()),
            schedule_id: None,
            parent_run_id: None,
            phase: Some("build".into()),
            agent_name: "build".into(),
            execution_mode: "BUILD_DIRECT".into(),
            provider_id: Some("opencode-go".into()),
            model_id: "glm-5.3".into(),
            effort_level: "low".into(),
            pricing_snapshot_json: None,
            log_path: Some(log_path.to_string_lossy().to_string()),
        },
    )
    .await
    .expect("run");

    let (status, list) = call(
        &ctx,
        Method::GET,
        &format!("/api/runs?workspace_id={}", ctx.workspace_id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(list.as_array().expect("runs").len(), 1);
    let (status, run) = call(&ctx, Method::GET, "/api/runs/run-api", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(run["task_title"], "Run API");
    let (status, log) = call(&ctx, Method::GET, "/api/runs/run-api/log", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(log.as_str().unwrap_or_default().contains("linha de log"));
    let (status, timeline) = call(&ctx, Method::GET, "/api/runs/run-api/timeline", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(timeline["run"].is_object());

    let artifact_file = ctx.workspace_root.join("output/run/entrega.md");
    std::fs::create_dir_all(artifact_file.parent().expect("parent")).expect("dir");
    std::fs::write(&artifact_file, "# entrega").expect("artifact");
    artifacts::create(
        &ctx.state.db,
        artifacts::CreateArtifactInput {
            workspace_id: ctx.workspace_id.clone(),
            task_id: Some(task_id.clone()),
            run_id: "run-api".into(),
            file_path: "output/run/entrega.md".into(),
            absolute_path: artifact_file.to_string_lossy().to_string(),
            file_type: "md".into(),
            file_size_bytes: 9,
            content_sha256: None,
            previous_sha256: None,
            action_type: "CREATED".into(),
            created_by_agent: "build".into(),
            creation_reason: "teste".into(),
            diff_patch: None,
        },
    )
    .await
    .expect("artifact");
    sqlx::query(
        "UPDATE task_runs SET status = 'SUCCESS',
            completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+1 second')
         WHERE id = 'run-api'",
    )
    .execute(&ctx.state.db)
    .await
    .expect("finish run");

    let (status, forked) = call(&ctx, Method::POST, "/api/runs/run-api/fork", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(forked["id"], task_id);
    assert!(
        forked["description"]
            .as_str()
            .unwrap_or_default()
            .contains("Fork a partir de run-api")
    );

    let (status, _) = call(&ctx, Method::GET, "/api/runs/missing/log", None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn artifact_rollback_reverts_created_and_modified_files() {
    let ctx = test_support::context().await;
    runs::create(
        &ctx.state.db,
        runs::CreateRunInput {
            id: "run-rollback".into(),
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

    let created_file = ctx.workspace_root.join("output/created.md");
    std::fs::create_dir_all(created_file.parent().expect("parent")).expect("dir");
    std::fs::write(&created_file, "novo").expect("file");
    let created = artifacts::create(
        &ctx.state.db,
        artifacts::CreateArtifactInput {
            workspace_id: ctx.workspace_id.clone(),
            task_id: None,
            run_id: "run-rollback".into(),
            file_path: "output/created.md".into(),
            absolute_path: created_file.to_string_lossy().to_string(),
            file_type: "md".into(),
            file_size_bytes: 4,
            content_sha256: None,
            previous_sha256: None,
            action_type: "CREATED".into(),
            created_by_agent: "build".into(),
            creation_reason: "teste".into(),
            diff_patch: None,
        },
    )
    .await
    .expect("created artifact");
    let (status, result) = call(
        &ctx,
        Method::POST,
        &format!("/api/artifacts/{}/rollback", created.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["action"], "CREATED");
    assert!(!created_file.exists());

    let modified_file = ctx.workspace_root.join("output/modified.md");
    std::fs::write(&modified_file, "novo").expect("file");
    let modified = artifacts::create(
        &ctx.state.db,
        artifacts::CreateArtifactInput {
            workspace_id: ctx.workspace_id.clone(),
            task_id: None,
            run_id: "run-rollback".into(),
            file_path: "output/modified.md".into(),
            absolute_path: modified_file.to_string_lossy().to_string(),
            file_type: "md".into(),
            file_size_bytes: 4,
            content_sha256: None,
            previous_sha256: None,
            action_type: "MODIFIED".into(),
            created_by_agent: "build".into(),
            creation_reason: "teste".into(),
            diff_patch: Some("--- a\n+++ b\n@@\n-antigo\n+novo\n".into()),
        },
    )
    .await
    .expect("modified artifact");
    let (status, result) = call(
        &ctx,
        Method::POST,
        &format!("/api/artifacts/{}/rollback", modified.id),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["action"], "MODIFIED");
    assert_eq!(
        std::fs::read_to_string(&modified_file).expect("file"),
        "antigo"
    );
}

#[tokio::test]
async fn attachments_accept_payloads_above_the_axum_default_limit() {
    use base64::Engine as _;

    let ctx = test_support::context().await;
    let query = format!("workspace_id={}", ctx.workspace_id);
    let payload = vec![b'a'; 3 * 1024 * 1024];
    let encoded = base64::engine::general_purpose::STANDARD.encode(&payload);
    let (status, saved) = call(
        &ctx,
        Method::POST,
        &format!("/api/attachments?{query}"),
        Some(json!({ "filename": "large.txt", "base64": encoded })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["size"], payload.len());
}

#[tokio::test]
async fn attachments_are_saved_and_deleted_inside_the_workspace() {
    let ctx = test_support::context().await;
    let query = format!("workspace_id={}", ctx.workspace_id);
    let (status, saved) = call(
        &ctx,
        Method::POST,
        &format!("/api/attachments?{query}"),
        Some(json!({
            "filename": "briefing final.md",
            "base64": "data:text/markdown;base64,aGVsbG8="
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let path = saved["path"].as_str().expect("path");
    assert!(path.starts_with("input/attachments/"));
    assert_eq!(saved["mime"], "text/markdown");
    assert!(ctx.workspace_root.join(path).is_file());

    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!(
            "/api/attachments?{query}&path={}",
            urlencoding::encode(path)
        ),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!ctx.workspace_root.join(path).exists());

    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/attachments?{query}"),
        Some(json!({ "filename": "x.txt", "base64": "not-base64" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, _) = call(
        &ctx,
        Method::DELETE,
        &format!("/api/attachments?{query}&path=../escape"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
