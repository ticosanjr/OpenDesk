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

    let (status, done) = call_with_token(
        &ctx,
        Method::POST,
        &format!("/api/nodes/jobs/{}/complete", job.id),
        &token,
        Some(json!({
            "status": "completed",
            "summary": "ok",
            "duration_ms": 1000,
            "usage": { "tokens_input": 10, "tokens_output": 5 }
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

    let (status, empty) =
        call_with_token(&ctx, Method::GET, "/api/nodes/next-job", &token, None).await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert!(empty["job"].is_null());
}
