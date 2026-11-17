use super::*;

async fn call_with_token(
    ctx: &TestContext,
    method: Method,
    uri: &str,
    token: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(AUTHORIZATION, format!("Bearer {token}"));
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let response = router(ctx.state.clone())
        .oneshot(
            builder
                .body(
                    body.map(|value| Body::from(value.to_string()))
                        .unwrap_or_else(Body::empty),
                )
                .expect("request"),
        )
        .await
        .expect("response");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn node_registration_heartbeat_and_token_scope() {
    let ctx = test_support::context().await;
    let (status, created) = call(
        &ctx,
        Method::POST,
        "/api/nodes",
        Some(json!({
            "name": "worker-1",
            "endpoint": "http://10.0.0.5:4096",
            "capabilities": ["opencode"]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let token = created["token"].as_str().expect("token");
    assert_eq!(created["node"]["online"], false);

    let (status, beat) = call_with_token(
        &ctx,
        Method::POST,
        "/api/nodes/heartbeat",
        token,
        Some(json!({ "status": "ONLINE", "capabilities": ["opencode", "codex"] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{beat}");
    assert_eq!(beat["online"], true);
    assert_eq!(beat["status"], "ONLINE");

    // A node token is not a user session: listing nodes stays local-only.
    let (status, _) = call_with_token(&ctx, Method::GET, "/api/nodes", token, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, _) = call_with_token(
        &ctx,
        Method::POST,
        "/api/nodes/heartbeat",
        "nope",
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rotates_the_node_token() {
    let ctx = test_support::context().await;
    let (_, created) = call(
        &ctx,
        Method::POST,
        "/api/nodes",
        Some(json!({ "name": "worker-3" })),
    )
    .await;
    let id = created["node"]["id"].as_str().expect("node id");
    let old = created["token"].as_str().expect("token").to_string();
    let (status, rotated) =
        call(&ctx, Method::POST, &format!("/api/nodes/{id}/rotate"), None).await;
    assert_eq!(status, StatusCode::OK, "{rotated}");
    let new = rotated["token"].as_str().expect("new token").to_string();
    assert_ne!(old, new);

    let (status, _) = call_with_token(
        &ctx,
        Method::POST,
        "/api/nodes/heartbeat",
        &old,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, body) = call_with_token(
        &ctx,
        Method::POST,
        "/api/nodes/heartbeat",
        &new,
        Some(json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn node_claims_leases_and_completes_a_job() {
    let ctx = test_support::context().await;
    let (_, created) = call(
        &ctx,
        Method::POST,
        "/api/nodes",
        Some(json!({ "name": "worker-2" })),
    )
    .await;
    let token = created["token"].as_str().expect("token").to_string();
    call_with_token(
        &ctx,
        Method::POST,
        "/api/nodes/heartbeat",
        &token,
        Some(json!({ "status": "ONLINE" })),
    )
    .await;

    let job = queue_db::enqueue(
        &ctx.state.db,
        queue_db::EnqueueInput {
            workspace_id: ctx.workspace_id.clone(),
            task_id: None,
            schedule_id: None,
            session_id: None,
            concurrency_group: None,
            prompt_payload: "prompt".into(),
            resolved_mode: "build".into(),
            resolved_agent: "build".into(),
            resolved_model: "opencode-go/glm-5.3".into(),
            resolved_effort: "medium".into(),
            resolved_harness: "opencode".into(),
            job_kind: "task".into(),
            scheduled_for: None,
            output_dir: None,
            priority: 2,
            max_retries: 1,
        },
    )
    .await
    .expect("job");

    let (status, claim) =
        call_with_token(&ctx, Method::GET, "/api/nodes/next-job", &token, None).await;
    assert_eq!(status, StatusCode::OK, "{claim}");
    assert_eq!(claim["job"]["id"], job.id);
    assert!(claim["run_id"].is_string());
    assert!(claim["workspace_root"].is_string());
    assert_eq!(claim["sandbox"]["mode"], "off");
    let run_id = claim["run_id"].as_str().expect("run id").to_string();

    let claimed = queue_db::get(&ctx.state.db, &job.id).await.expect("queue");
    assert_eq!(claimed.status, "RUNNING");
    assert!(claimed.assigned_node_id.is_some());

    let (status, beat) = call_with_token(
        &ctx,
        Method::POST,
        &format!("/api/nodes/jobs/{}/heartbeat", job.id),
        &token,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{beat}");

    std::fs::write(ctx.workspace_root.join("worker-out.txt"), "artefato remoto")
        .expect("workspace file");
    let (status, done) = call_with_token(
        &ctx,
        Method::POST,
        &format!("/api/nodes/jobs/{}/complete", job.id),
        &token,
        Some(json!({
            "status": "completed",
            "summary": "ok",
            "duration_ms": 1000,
            "usage": { "tokens_input": 10, "tokens_output": 5 },
            "files": [{ "path": "worker-out.txt", "status": "added", "patch": "diff" }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["queue_status"], "COMPLETED");
    assert_eq!(
        queue_db::get(&ctx.state.db, &job.id)
            .await
            .expect("queue")
            .status,
        "COMPLETED"
    );
    let (status, artifacts) = call(
        &ctx,
        Method::GET,
        &format!("/api/artifacts?run_id={run_id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{artifacts}");
    assert_eq!(artifacts.as_array().map(Vec::len), Some(1));
    assert_eq!(artifacts[0]["file_path"], "worker-out.txt");

    let (status, empty) =
        call_with_token(&ctx, Method::GET, "/api/nodes/next-job", &token, None).await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert!(empty["job"].is_null());
}

#[tokio::test]
async fn node_sync_downloads_a_bundle_and_uploads_changed_content() {
    let ctx = test_support::context().await;
    std::fs::write(ctx.workspace_root.join("base.txt"), "base\n").expect("base");
    let (_, created) = call(
        &ctx,
        Method::POST,
        "/api/nodes",
        Some(json!({ "name": "sync-worker" })),
    )
    .await;
    let token = created["token"].as_str().expect("token").to_string();
    let job = crate::db::queue::enqueue(
        &ctx.state.db,
        crate::db::queue::EnqueueInput {
            workspace_id: ctx.workspace_id.clone(),
            task_id: None,
            schedule_id: None,
            session_id: None,
            concurrency_group: Some("light_api".into()),
            prompt_payload: "sync test".into(),
            resolved_mode: "BUILD_DIRECT".into(),
            resolved_agent: "build".into(),
            resolved_model: "model/test".into(),
            resolved_effort: "medium".into(),
            resolved_harness: "opencode".into(),
            job_kind: "task".into(),
            scheduled_for: None,
            output_dir: None,
            priority: 2,
            max_retries: 0,
        },
    )
    .await
    .expect("job");
    let (status, claim) =
        call_with_token(&ctx, Method::GET, "/api/nodes/next-job", &token, None).await;
    assert_eq!(status, StatusCode::OK, "{claim}");
    assert_eq!(claim["job"]["id"], job.id);
    let run_id = claim["run_id"].as_str().expect("run").to_string();

    let (status, sync) = call_with_token(
        &ctx,
        Method::POST,
        &format!("/api/nodes/jobs/{}/sync", job.id),
        &token,
        Some(json!({ "base": [] })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sync}");
    let bundle_id = sync["bundle_id"].as_str().expect("bundle").to_string();
    assert!(sync["files"].as_u64().unwrap_or(0) >= 1, "{sync}");

    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/nodes/jobs/{}/sync/{}", job.id, bundle_id))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(request)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .expect("bytes");
    let temp = tempfile::TempDir::new().expect("temp");
    let bundle = temp.path().join("bundle.tar.zst");
    std::fs::write(&bundle, bytes).expect("bundle");
    crate::workspace_sync::apply(&bundle, temp.path(), false).expect("apply");
    assert_eq!(
        std::fs::read_to_string(temp.path().join("base.txt")).expect("base"),
        "base\n"
    );

    std::fs::write(temp.path().join("changed.txt"), "novo\n").expect("changed");
    let content = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, "novo\n");
    let (status, done) = call_with_token(
        &ctx,
        Method::POST,
        &format!("/api/nodes/jobs/{}/complete", job.id),
        &token,
        Some(json!({
            "status": "completed",
            "run_id": run_id,
            "duration_ms": 10,
            "files": [{
                "path": "changed.txt",
                "status": "added",
                "patch": "",
                "content_base64": content
            }]
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(
        std::fs::read_to_string(ctx.workspace_root.join("changed.txt")).expect("uploaded"),
        "novo\n"
    );
}

#[tokio::test]
async fn expired_node_lease_is_reclaimed_without_duplicating_the_job() {
    let ctx = test_support::context().await;
    let (_, first) = call(
        &ctx,
        Method::POST,
        "/api/nodes",
        Some(json!({ "name": "node-a" })),
    )
    .await;
    let (_, second) = call(
        &ctx,
        Method::POST,
        "/api/nodes",
        Some(json!({ "name": "node-b" })),
    )
    .await;
    let token_a = first["token"].as_str().expect("token a");
    let token_b = second["token"].as_str().expect("token b");
    let job = crate::db::queue::enqueue(
        &ctx.state.db,
        crate::db::queue::EnqueueInput {
            workspace_id: ctx.workspace_id.clone(),
            task_id: None,
            schedule_id: None,
            session_id: None,
            concurrency_group: None,
            prompt_payload: "lease".into(),
            resolved_mode: "BUILD_DIRECT".into(),
            resolved_agent: "build".into(),
            resolved_model: "model/test".into(),
            resolved_effort: "medium".into(),
            resolved_harness: "opencode".into(),
            job_kind: "task".into(),
            scheduled_for: None,
            output_dir: None,
            priority: 2,
            max_retries: 1,
        },
    )
    .await
    .expect("job");
    let (status, claim) =
        call_with_token(&ctx, Method::GET, "/api/nodes/next-job", token_a, None).await;
    assert_eq!(status, StatusCode::OK, "{claim}");
    assert_eq!(claim["job"]["id"], job.id);
    sqlx::query(
        "UPDATE execution_queue SET lease_expires_at = datetime('now','-1 minute') WHERE id = ?",
    )
    .bind(&job.id)
    .execute(&ctx.state.db)
    .await
    .expect("expire");
    let (status, reclaim) =
        call_with_token(&ctx, Method::GET, "/api/nodes/next-job", token_b, None).await;
    assert_eq!(status, StatusCode::OK, "{reclaim}");
    assert_eq!(reclaim["job"]["id"], job.id, "{reclaim}");
    assert_eq!(reclaim["job"]["retry_count"], 1, "{reclaim}");
    let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM execution_queue WHERE id = ?")
        .bind(&job.id)
        .fetch_one(&ctx.state.db)
        .await
        .expect("count");
    assert_eq!(rows, 1);
}
