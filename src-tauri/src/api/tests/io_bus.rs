use super::*;

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
async fn io_bus_bindings_secrets_and_schedule_sinks_round_trip() {
    let ctx = test_support::context().await;
    let query = format!("?workspace_id={}", ctx.workspace_id);
    let (_, task) = call(
        &ctx,
        Method::POST,
        &format!("/api/tasks?workspace_id={}", ctx.workspace_id),
        Some(json!({ "title": "Sinks", "description": "teste" })),
    )
    .await;
    let task_id = task["id"].as_str().expect("task");
    let (_, schedule) = call(
        &ctx,
        Method::POST,
        "/api/schedules",
        Some(json!({
            "workspace_id": ctx.workspace_id,
            "name": "Sinks",
            "command_or_prompt": "executar",
            "cron_expression": "0 9 * * *",
            "timezone": "UTC"
        })),
    )
    .await;
    let schedule_id = schedule["id"].as_str().expect("schedule");
    let (status, sink) = call(
        &ctx,
        Method::POST,
        &format!("/api/io/outbound{query}"),
        Some(json!({
            "name": "Sink QA",
            "sink_type": "ATOMIC_FS",
            "config": { "path": "output" },
            "is_enabled": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let sink_id = sink["id"].as_str().expect("sink").to_string();

    for uri in [
        format!("/api/tasks/{task_id}/sinks"),
        format!("/api/schedules/{schedule_id}/sinks"),
    ] {
        let (status, _) = call(
            &ctx,
            Method::POST,
            &uri,
            Some(json!({ "sink_ids": [sink_id] })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, bindings) = call(&ctx, Method::GET, &uri, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(bindings["bindings"].as_array().expect("bindings").len(), 1);
    }

    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/io/secrets",
        Some(json!({ "key": "qa-key", "value": "secret" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&ctx, Method::DELETE, "/api/io/secrets/qa-key", None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/io/secrets",
        Some(json!({ "key": "", "value": "x" })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
