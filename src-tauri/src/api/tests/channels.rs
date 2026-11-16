use super::*;

async fn inbound(
    ctx: &TestContext,
    channel: &str,
    token: Option<&str>,
    body: Value,
) -> (StatusCode, Value) {
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/webhooks/channels/{channel}"))
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("x-opendesk-channel-token", token);
    }
    let response = router(ctx.state.clone())
        .oneshot(builder.body(Body::from(body.to_string())).expect("request"))
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
async fn creates_and_lists_routes() {
    let ctx = test_support::context().await;
    let (status, route) = call(
        &ctx,
        Method::POST,
        "/api/channels/routes",
        Some(json!({
            "channel": "telegram",
            "identity": "42",
            "agent": "build",
            "permission_profile": "default",
            "trust_level": "internal"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{route}");
    assert_eq!(route["trust_level"], "internal");

    let (status, list) = call(&ctx, Method::GET, "/api/channels/routes", None).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    assert_eq!(list.as_array().map(Vec::len), Some(1));

    let (status, statuses) = call(&ctx, Method::GET, "/api/channels/status", None).await;
    assert_eq!(status, StatusCode::OK, "{statuses}");
    assert!(
        statuses
            .as_array()
            .expect("statuses")
            .iter()
            .any(|item| item["channel"] == "telegram" && item["routes"] == 1)
    );
}

#[tokio::test]
async fn untrusted_messages_create_only_a_ticket() {
    let ctx = test_support::context().await;
    call(
        &ctx,
        Method::POST,
        "/api/channels/routes",
        Some(json!({
            "channel": "whatsapp",
            "identity": "*",
            "agent": "build",
            "permission_profile": "read_only",
            "trust_level": "untrusted"
        })),
    )
    .await;
    let (status, token) = call(
        &ctx,
        Method::POST,
        "/api/channels/whatsapp/webhook-token",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{token}");
    let secret = token["token"].as_str().expect("token").to_string();

    let (status, body) = inbound(
        &ctx,
        "whatsapp",
        Some(&secret),
        json!({
            "identity": "5511999990000",
            "text": "Oi, preciso de ajuda com a proposta",
            "message_id": "wamid-1"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["ticket_id"].is_string());
    assert!(body["task_id"].is_null());
    assert_eq!(body["trust_level"], "untrusted");

    let (_, tickets) = call(&ctx, Method::GET, "/api/tickets", None).await;
    assert_eq!(tickets.as_array().map(Vec::len), Some(1));
    let (_, tasks) = call(&ctx, Method::GET, "/api/tasks", None).await;
    assert_eq!(tasks.as_array().map(Vec::len), Some(0));

    // Same provider message id is idempotent.
    let (status, again) = inbound(
        &ctx,
        "whatsapp",
        Some(&secret),
        json!({
            "identity": "5511999990000",
            "text": "Oi, preciso de ajuda com a proposta",
            "message_id": "wamid-1"
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["ticket_id"], body["ticket_id"]);
}

#[tokio::test]
async fn trusted_messages_create_a_task_and_reject_bad_tokens() {
    let ctx = test_support::context().await;
    call(
        &ctx,
        Method::POST,
        "/api/channels/routes",
        Some(json!({
            "channel": "telegram",
            "identity": "42",
            "agent": "build",
            "permission_profile": "default",
            "trust_level": "trusted"
        })),
    )
    .await;
    let (_, token) = call(
        &ctx,
        Method::POST,
        "/api/channels/telegram/webhook-token",
        None,
    )
    .await;
    let secret = token["token"].as_str().expect("token").to_string();

    let (status, _) = inbound(
        &ctx,
        "telegram",
        Some("wrong"),
        json!({ "identity": "42", "text": "oi" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, body) = inbound(
        &ctx,
        "telegram",
        Some(&secret),
        json!({ "identity": "42", "text": "gere o relatório" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["task_id"].is_string());
    assert_eq!(body["held"], false);

    let (_, tasks) = call(&ctx, Method::GET, "/api/tasks", None).await;
    let tasks = tasks.as_array().expect("tasks");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["origin"], "CHANNEL");
}

#[tokio::test]
async fn rejects_messages_without_a_route_or_token() {
    let ctx = test_support::context().await;
    let (status, _) = inbound(
        &ctx,
        "email",
        Some("x"),
        json!({ "identity": "a@b.c", "text": "oi" }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (_, token) = call(
        &ctx,
        Method::POST,
        "/api/channels/email/webhook-token",
        None,
    )
    .await;
    let secret = token["token"].as_str().expect("token").to_string();
    let (status, _) = inbound(
        &ctx,
        "email",
        Some(&secret),
        json!({ "identity": "a@b.c", "text": "oi" }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
