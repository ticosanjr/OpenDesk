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
            "config": { "secret": "segredo-qa" },
            "auto_execute": false,
            "is_enabled": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let inbound_id = inbound["id"].as_str().expect("inbound").to_string();
    let stored: Value =
        serde_json::from_str(inbound["config_json"].as_str().expect("config")).expect("json");
    assert!(stored["secret_ref"].is_string(), "{stored}");
    assert!(
        stored.get("secret").is_none(),
        "segredo em texto puro: {stored}"
    );
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
            "config": { "secret": "segredo-qa-2" },
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

async fn post_webhook(
    ctx: &TestContext,
    id: &str,
    body: &str,
    signature: Option<&str>,
) -> StatusCode {
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/webhooks/{id}"))
        .header(AUTHORIZATION, format!("Bearer {}", ctx.state.api_token))
        .header("content-type", "application/json");
    if let Some(signature) = signature {
        builder = builder.header("x-opendesk-signature", signature);
    }
    let request = builder.body(Body::from(body.to_string())).expect("request");
    router(ctx.state.clone())
        .oneshot(request)
        .await
        .expect("response")
        .status()
}

fn hmac_hex(secret: &str, body: &str) -> String {
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::Sha256;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("hmac");
    mac.update(body.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[tokio::test]
async fn webhook_requires_signature_and_an_enabled_source() {
    let ctx = test_support::context().await;
    let query = format!("?workspace_id={}", ctx.workspace_id);
    let secret = "segredo-webhook-teste";
    let (status, source) = call(
        &ctx,
        Method::POST,
        &format!("/api/io/inbound{query}"),
        Some(json!({
            "name": "Webhook assinado",
            "source_type": "WEBHOOK_HTTP",
            "config": { "secret": secret },
            "auto_execute": false,
            "is_enabled": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = source["id"].as_str().expect("source").to_string();
    let body = r#"{"title":"Chamado","body":"Corpo"}"#;

    assert_eq!(
        post_webhook(&ctx, &id, body, None).await,
        StatusCode::BAD_REQUEST,
        "sem assinatura"
    );
    assert_eq!(
        post_webhook(&ctx, &id, body, Some("00")).await,
        StatusCode::BAD_REQUEST,
        "assinatura inválida"
    );
    let signature = hmac_hex(secret, body);
    assert_eq!(
        post_webhook(&ctx, &id, body, Some(&signature)).await,
        StatusCode::OK,
        "assinatura válida"
    );

    let (status, disabled) = call(
        &ctx,
        Method::POST,
        &format!("/api/io/inbound{query}"),
        Some(json!({
            "name": "Webhook desativado",
            "source_type": "WEBHOOK_HTTP",
            "config": { "secret": secret },
            "auto_execute": false,
            "is_enabled": false
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let disabled_id = disabled["id"].as_str().expect("source");
    let signature = hmac_hex(secret, body);
    assert_eq!(
        post_webhook(&ctx, disabled_id, body, Some(&signature)).await,
        StatusCode::BAD_REQUEST,
        "fonte desativada"
    );
}

#[tokio::test]
async fn webhook_source_without_secret_is_rejected() {
    let ctx = test_support::context().await;
    let query = format!("?workspace_id={}", ctx.workspace_id);
    let (status, _) = call(
        &ctx,
        Method::POST,
        &format!("/api/io/inbound{query}"),
        Some(json!({
            "name": "Webhook aberto",
            "source_type": "WEBHOOK_HTTP",
            "config": {},
            "auto_execute": false,
            "is_enabled": true
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
