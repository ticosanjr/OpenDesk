use super::*;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

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

#[tokio::test]
async fn whatsapp_meta_verification_and_signature() {
    let ctx = test_support::context().await;
    ctx.state
        .secrets
        .set("channel_webhook", "whatsapp", "verify-token")
        .expect("webhook token");
    ctx.state
        .secrets
        .set("channel_whatsapp", "app_secret", "app-secret")
        .expect("app secret");

    let request = Request::builder()
        .uri("/api/webhooks/channels/whatsapp?hub.mode=subscribe&hub.verify_token=verify-token&hub.challenge=abc123")
        .body(Body::empty())
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(request)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024).await.expect("body");
    assert_eq!(String::from_utf8_lossy(&body), "abc123");

    let request = Request::builder()
        .uri("/api/webhooks/channels/whatsapp?hub.mode=subscribe&hub.verify_token=wrong&hub.challenge=x")
        .body(Body::empty())
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(request)
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

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
    let payload = json!({
        "object": "whatsapp_business_account",
        "entry": [{
            "changes": [{
                "value": {
                    "contacts": [{ "wa_id": "5511999", "profile": { "name": "Ana" } }],
                    "messages": [{
                        "from": "5511999",
                        "id": "wamid.meta-1",
                        "type": "text",
                        "text": { "body": "Olá pelo WhatsApp" }
                    }]
                }
            }]
        }]
    });
    let bytes = serde_json::to_vec(&payload).expect("payload");
    let mut mac = Hmac::<Sha256>::new_from_slice(b"app-secret").expect("mac");
    mac.update(&bytes);
    let signature = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/webhooks/channels/whatsapp")
        .header("content-type", "application/json")
        .header("x-hub-signature-256", format!("sha256={signature}"))
        .body(Body::from(bytes))
        .expect("request");
    let response = router(ctx.state.clone())
        .oneshot(request)
        .await
        .expect("response");
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body");
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let value: Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(value["accepted"], 1);
    let (_, tickets) = call(&ctx, Method::GET, "/api/tickets", None).await;
    assert_eq!(tickets.as_array().map(Vec::len), Some(1));
}

#[tokio::test]
async fn whatsapp_cloud_api_requires_configuration() {
    let ctx = test_support::context().await;
    let (status, body) = call(
        &ctx,
        Method::PUT,
        "/api/channels/whatsapp",
        Some(json!({
            "enabled": true,
            "provider": "cloud-api",
            "phone_number": "+55",
            "notes": ""
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    let (status, body) = call(
        &ctx,
        Method::PUT,
        "/api/channels/whatsapp",
        Some(json!({
            "enabled": true,
            "provider": "cloud-api",
            "phone_number": "+55",
            "phone_number_id": "123",
            "api_version": "v21.0",
            "access_token": "token",
            "app_secret": "secret"
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["connected"], true);
    assert_eq!(body["access_token_set"], true);

    // Sending is only possible while the access token exists; this keeps the
    // test hermetic and avoids calling Meta.
    ctx.state
        .secrets
        .delete("channel_whatsapp", "access_token")
        .expect("delete token");
    let (status, _) = call(
        &ctx,
        Method::POST,
        "/api/channels/whatsapp/send",
        Some(json!({ "to": "5511999", "text": "oi" })),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}
