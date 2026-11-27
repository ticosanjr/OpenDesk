use super::*;

#[tokio::test]
async fn http_mcp_gateway_initializes_and_lists_tools() {
    let ctx = test_support::context().await;
    let path = format!("/api/mcp?workspace_id={}", ctx.workspace_id);
    let (status, init) = call(
        &ctx,
        Method::POST,
        &path,
        Some(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{init}");
    assert_eq!(init["result"]["serverInfo"]["name"], "opendesk-control");

    let (status, list) = call(
        &ctx,
        Method::POST,
        &path,
        Some(json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert!(
        list["result"]["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .any(|tool| tool["name"] == "task_list"),
        "{list}"
    );
}

#[tokio::test]
async fn role_read_hides_build_tools() {
    let ctx = test_support::context().await;
    let request = axum::http::Request::builder()
        .method(Method::POST)
        .uri(format!("/api/mcp?workspace_id={}", ctx.workspace_id))
        .header(AUTHORIZATION, format!("Bearer {}", ctx.state.api_token))
        .header("content-type", "application/json")
        .header("x-opendesk-mcp-role", "read")
        .body(axum::body::Body::from(
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/list",
                "params": {}
            })
            .to_string(),
        ))
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(request)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    let body: Value = serde_json::from_slice(&bytes).expect("json");
    let tools = body["result"]["tools"].as_array().expect("tools");
    assert!(tools.iter().any(|tool| tool["name"] == "task_list"));
    assert!(!tools.iter().any(|tool| tool["name"] == "task_create"));
}

#[tokio::test]
async fn agent_policy_limits_tools_and_calls() {
    let ctx = test_support::context().await;
    crate::db::settings::set(
        &ctx.state.db,
        "mcp_agent_tools",
        &json!({ "build": { "allow": ["task_list"], "deny": ["task_delete"] } }),
    )
    .await
    .expect("setting");
    let request = axum::http::Request::builder()
        .method(Method::POST)
        .uri(format!("/api/mcp?workspace_id={}", ctx.workspace_id))
        .header(AUTHORIZATION, format!("Bearer {}", ctx.state.api_token))
        .header("content-type", "application/json")
        .header("x-opendesk-mcp-agent", "build")
        .body(axum::body::Body::from(
            json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/list",
                "params": {}
            })
            .to_string(),
        ))
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(request)
        .await
        .expect("response");
    let bytes = axum::body::to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .expect("body");
    let body: Value = serde_json::from_slice(&bytes).expect("json");
    let tools = body["result"]["tools"].as_array().expect("tools");
    assert_eq!(tools.len(), 1, "{body}");
    assert_eq!(tools[0]["name"], "task_list");
}

#[tokio::test]
async fn streamable_http_creates_resumes_and_deletes_a_session() {
    let ctx = test_support::context().await;
    let path = format!("/api/mcp?workspace_id={}", ctx.workspace_id);
    let init = axum::http::Request::builder()
        .method(Method::POST)
        .uri(&path)
        .header(AUTHORIZATION, format!("Bearer {}", ctx.state.api_token))
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {}
            })
            .to_string(),
        ))
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(init)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let session = response
        .headers()
        .get("mcp-session-id")
        .expect("session header")
        .to_str()
        .expect("header")
        .to_string();

    let sse = axum::http::Request::builder()
        .method(Method::POST)
        .uri(&path)
        .header(AUTHORIZATION, format!("Bearer {}", ctx.state.api_token))
        .header("content-type", "application/json")
        .header("accept", "text/event-stream")
        .header("mcp-session-id", &session)
        .body(axum::body::Body::from(
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            })
            .to_string(),
        ))
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(sse)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream")
    );
    let bytes = axum::body::to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .expect("body");
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("event: message"), "{text}");
    assert!(text.contains("task_list"), "{text}");

    let notification = axum::http::Request::builder()
        .method(Method::POST)
        .uri(&path)
        .header(AUTHORIZATION, format!("Bearer {}", ctx.state.api_token))
        .header("content-type", "application/json")
        .header("mcp-session-id", &session)
        .body(axum::body::Body::from(
            json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized",
                "params": {}
            })
            .to_string(),
        ))
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(notification)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let stream = axum::http::Request::builder()
        .method(Method::GET)
        .uri(&path)
        .header(AUTHORIZATION, format!("Bearer {}", ctx.state.api_token))
        .header("accept", "text/event-stream")
        .header("mcp-session-id", &session)
        .body(axum::body::Body::empty())
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(stream)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok()),
        Some("text/event-stream")
    );

    let delete = axum::http::Request::builder()
        .method(Method::DELETE)
        .uri(&path)
        .header(AUTHORIZATION, format!("Bearer {}", ctx.state.api_token))
        .header("mcp-session-id", &session)
        .body(axum::body::Body::empty())
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(delete)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(ctx.state.mcp_sessions.get(&session).is_none());
}

#[tokio::test]
async fn rate_limits_and_audits_tool_calls() {
    let ctx = test_support::context().await;
    crate::db::settings::set(&ctx.state.db, "mcp_tool_rates", &json!({ "default": 1 }))
        .await
        .expect("setting");
    let path = format!("/api/mcp?workspace_id={}", ctx.workspace_id);
    let call_tool = |id: u64| {
        (
            format!("/api/mcp?workspace_id={}", ctx.workspace_id),
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": "tools/call",
                "params": { "name": "mcp_discover_tools", "arguments": { "query": "" } }
            }),
        )
    };
    let (path1, body1) = call_tool(1);
    let (status, first) = call(&ctx, Method::POST, &path1, Some(body1)).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert!(first.get("result").is_some(), "{first}");
    let (path2, body2) = call_tool(2);
    let (status, second) = call(&ctx, Method::POST, &path2, Some(body2)).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["error"]["code"], -32002, "{second}");
    let _ = path;
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT action, status FROM api_audit WHERE action = 'mcp:mcp_discover_tools' ORDER BY created_at",
    )
    .fetch_all(&ctx.state.db)
    .await
    .expect("audit");
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0].1, 200);
    assert_eq!(rows[1].1, 429);
}

#[tokio::test]
async fn proxies_external_stdio_tools_with_a_namespace() {
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let ctx = test_support::context().await;
    let script = ctx.workspace_root.join("fake_mcp.py");
    std::fs::write(
        &script,
        r#"import json, sys
for line in sys.stdin:
    req = json.loads(line)
    method = req.get("method")
    if method == "initialize":
        result = {"protocolVersion": "2024-11-05", "capabilities": {"tools": {}}, "serverInfo": {"name": "fake", "version": "1"}}
    elif method == "tools/list":
        result = {"tools": [{"name": "echo", "description": "echo", "inputSchema": {"type": "object"}}]}
    elif method == "tools/call":
        result = {"content": [{"type": "text", "text": json.dumps(req.get("params", {}).get("arguments", {}))}], "isError": False}
    else:
        result = {}
    print(json.dumps({"jsonrpc": "2.0", "id": req.get("id"), "result": result}), flush=True)
"#,
    )
    .expect("script");
    std::fs::write(
        ctx.workspace_root.join("opencode.jsonc"),
        format!(
            r#"{{"mcp":{{"servers":{{"fake":{{"type":"local","command":["python3","{}"]}}}}}}}}"#,
            script.display()
        ),
    )
    .expect("config");
    let path = format!("/api/mcp?workspace_id={}", ctx.workspace_id);
    let (status, list) = call(
        &ctx,
        Method::POST,
        &path,
        Some(json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/list",
            "params": {}
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert!(
        list["result"]["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .any(|tool| tool["name"] == "fake__echo"),
        "{list}"
    );
    let (status, called) = call(
        &ctx,
        Method::POST,
        &path,
        Some(json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "tools/call",
            "params": { "name": "fake__echo", "arguments": { "text": "oi" } }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{called}");
    assert!(
        called["result"]["content"][0]["text"]
            .as_str()
            .unwrap_or_default()
            .contains("oi"),
        "{called}"
    );
}

#[tokio::test]
async fn elicit_without_session_returns_actionable_error() {
    let ctx = test_support::context().await;
    let path = format!("/api/mcp?workspace_id={}", ctx.workspace_id);
    let (status, body) = call(
        &ctx,
        Method::POST,
        &path,
        Some(json!({
            "jsonrpc": "2.0",
            "id": 20,
            "method": "tools/call",
            "params": { "name": "elicit", "arguments": { "message": "nome?" } }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["error"]["code"], -32003, "{body}");
}

#[tokio::test]
async fn sampling_respects_budget_hard_stop() {
    let ctx = test_support::context().await;
    crate::db::budget::update(
        &ctx.state.db,
        crate::db::budget::UpdateBudgetInput {
            daily_limit_usd: Some(0.0001),
            monthly_limit_usd: Some(0.0001),
            soft_alert_threshold_pct: None,
            hard_stop_enabled: Some(true),
            opencode_go_5h_token_limit: None,
        },
    )
    .await
    .expect("budget");
    sqlx::query(
        "INSERT INTO task_runs
            (id, workspace_id, agent_name, execution_mode, model_id, effort_level,
             status, estimated_cost_usd)
         VALUES ('run-sampling', ?, 'build', 'BUILD_DIRECT', 'model', 'medium',
             'SUCCESS', 1.0)",
    )
    .bind(&ctx.workspace_id)
    .execute(&ctx.state.db)
    .await
    .expect("run");
    let path = format!("/api/mcp?workspace_id={}", ctx.workspace_id);
    let (status, body) = call(
        &ctx,
        Method::POST,
        &path,
        Some(json!({
            "jsonrpc": "2.0",
            "id": 21,
            "method": "tools/call",
            "params": { "name": "sampling", "arguments": { "prompt": "oi" } }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["error"]["code"], -32004, "{body}");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase()
            .contains("hard cap"),
        "{body}"
    );
}

#[tokio::test]
async fn invalid_memory_scope_returns_json_rpc_invalid_params() {
    let ctx = test_support::context().await;
    let path = format!("/api/mcp?workspace_id={}", ctx.workspace_id);
    let (status, body) = call(
        &ctx,
        Method::POST,
        &path,
        Some(json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "tools/call",
            "params": {
                "name": "memory_set_scope",
                "arguments": { "id": "missing", "usage_scope": "none" }
            }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["error"]["code"], -32602, "{body}");
    assert!(
        body["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("usage_scope"),
        "{body}"
    );

    let (_, body) = call(
        &ctx,
        Method::POST,
        &path,
        Some(json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/call",
            "params": {
                "name": "personal_context_set_scope",
                "arguments": { "id": "missing", "usage_scope": "invalido" }
            }
        })),
    )
    .await;
    assert_eq!(body["error"]["code"], -32602, "{body}");
}
