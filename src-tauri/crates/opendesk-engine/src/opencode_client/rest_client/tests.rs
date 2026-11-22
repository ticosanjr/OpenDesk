use super::*;
use axum::{
    Json, Router,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
};

fn authorized(headers: &HeaderMap) -> bool {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .map(|value| value == "Basic b3BlbmNvZGU6c2VjcmV0")
        .unwrap_or(false)
}

async fn mock_server() -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route(
            "/api/info",
            get(|headers: HeaderMap| async move {
                if !authorized(&headers) {
                    return StatusCode::UNAUTHORIZED.into_response();
                }
                Json(serde_json::json!({ "version": "2.2.5", "pid": 42 })).into_response()
            }),
        )
        .route(
            "/api/model",
            get(|headers: HeaderMap| async move {
                if !authorized(&headers) {
                    return StatusCode::UNAUTHORIZED.into_response();
                }
                Json(serde_json::json!({
                    "data": [{
                        "id": "glm",
                        "modelID": "glm-5.3",
                        "providerID": "opencode-go",
                        "cost": [{ "input": 1.0, "output": 2.0 }]
                    }]
                }))
                .into_response()
            }),
        )
        .route(
            "/api/skill",
            get(|| async {
                Json(serde_json::json!({
                    "data": [{ "id": "opencode", "name": "OpenCode", "description": "Docs" }]
                }))
            }),
        )
        .route(
            "/api/command",
            get(|| async {
                Json(serde_json::json!({
                    "data": [{ "name": "init", "description": "guided setup" }]
                }))
            }),
        )
        .route(
            "/api/mcp",
            get(|| async {
                Json(serde_json::json!({
                    "data": [{ "name": "rea", "status": { "status": "connected" } }]
                }))
            }),
        )
        .route(
            "/api/agent",
            get(|headers: HeaderMap| async move {
                if !authorized(&headers) {
                    return StatusCode::UNAUTHORIZED.into_response();
                }
                Json(serde_json::json!({ "data": [{ "id": "build", "permissions": {} }] }))
                    .into_response()
            }),
        )
        .route(
            "/api/session",
            post(|headers: HeaderMap, Json(_body): Json<Value>| async move {
                if !authorized(&headers) {
                    return StatusCode::UNAUTHORIZED.into_response();
                }
                Json(serde_json::json!({ "data": { "id": "ses-1" } })).into_response()
            }),
        )
        .route(
            "/api/experimental/generate/text",
            post(|Json(_body): Json<Value>| async {
                Json(serde_json::json!({ "data": { "text": "OK" } }))
            }),
        )
        .route(
            "/api/session/{id}/prompt",
            post(|| async { Json(serde_json::json!({ "data": { "id": "msg-user" } })) }),
        )
        .route(
            "/api/session/{id}/interrupt",
            post(|| async { StatusCode::NO_CONTENT }),
        )
        .route(
            "/api/experimental/session/{id}/wait",
            post(|| async { StatusCode::NO_CONTENT }),
        )
        .route(
            "/api/session/{id}/message",
            get(|| async { Json(serde_json::json!({ "data": [] })) }),
        )
        .route(
            "/api/session/{id}/diff",
            get(|| async {
                Json(serde_json::json!({
                    "data": [{ "file": "a.md", "patch": "+x", "status": "added" }]
                }))
            }),
        )
        .route(
            "/api/session/{id}/permission/{request}/reply",
            post(|| async { StatusCode::NO_CONTENT }),
        )
        .route(
            "/api/session/{id}/compact",
            post(|| async { Json(serde_json::json!({ "data": { "id": "cmp-1" } })) }),
        )
        .route(
            "/api/session/{id}/context",
            get(|| async { Json(serde_json::json!({ "data": [] })) }),
        )
        .route(
            "/api/event",
            get(|| async { ([("content-type", "text/event-stream")], "data: ok\n\n") }),
        );

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{address}"), handle)
}

#[tokio::test]
async fn exercises_the_v2_rest_surface_with_basic_auth() {
    let (url, handle) = mock_server().await;
    let client = RestClient::new_with_url(&url, "secret".into());
    assert_eq!(
        client.base_url().trim_end_matches('/'),
        url.trim_end_matches('/')
    );

    let info = client.info().await.expect("info");
    assert_eq!(info.version, "2.2.5");
    assert_eq!(info.pid, 42);

    let models = client.models(Some("/tmp/ws")).await.expect("models");
    assert_eq!(models[0].model_id.as_deref(), Some("glm-5.3"));
    let agents = client.agents(None).await.expect("agents");
    assert_eq!(agents[0].id, "build");
    let skills = client.skills(None).await.expect("skills");
    assert_eq!(skills[0]["id"], "opencode");
    let commands = client.commands(None).await.expect("commands");
    assert_eq!(commands[0]["name"], "init");
    let mcps = client.mcp_servers(None).await.expect("mcps");
    assert_eq!(mcps[0]["name"], "rea");

    let generated = client
        .generate_text(
            "Output only: OK",
            Some(ModelRefDto::parse("opencode-go/glm-5.3").expect("model")),
        )
        .await
        .expect("generate");
    assert_eq!(generated, "OK");

    let session = client
        .create_session("/tmp/ws", "Test", "build", None)
        .await
        .expect("session");
    assert_eq!(session, "ses-1");
    client.prompt(&session, "oi").await.expect("prompt");
    client.interrupt(&session).await.expect("interrupt");
    client.wait_session(&session).await.expect("wait");

    let messages = client.messages(&session).await.expect("messages");
    assert!(messages.get("data").is_some());
    let diff = client.diff(&session).await.expect("diff");
    assert_eq!(diff[0].file, "a.md");
    client
        .reply_permission(&session, "req-1", "once")
        .await
        .expect("permission");
    let compacted = client.compact_session(&session).await.expect("compact");
    assert_eq!(
        compacted.pointer("/data/id").and_then(Value::as_str),
        Some("cmp-1")
    );
    assert!(client.context(&session).await.expect("context").is_empty());
    let event = client.event_response().await.expect("event");
    assert!(event.status().is_success());

    handle.abort();
}

#[test]
fn parses_and_serializes_model_refs_without_null_variant() {
    let plain = ModelRefDto::parse("opencode-go/glm-5.3").expect("plain");
    assert_eq!(plain.provider_id, "opencode-go");
    assert_eq!(plain.id, "glm-5.3");
    assert_eq!(plain.variant, None);
    assert_eq!(
        plain.to_payload().expect("payload"),
        serde_json::json!({"providerID": "opencode-go", "id": "glm-5.3"})
    );

    let variant = ModelRefDto::parse(" opencode-go/glm-5.3#high ").expect("variant");
    assert_eq!(variant.variant.as_deref(), Some("high"));
    assert_eq!(
        variant.to_payload().expect("payload"),
        serde_json::json!({"providerID": "opencode-go", "id": "glm-5.3", "variant": "high"})
    );

    let empty_variant = ModelRefDto::parse("opencode-go/glm-5.3#  ").expect("empty variant");
    assert_eq!(empty_variant.variant, None);
    assert_eq!(
        empty_variant.to_payload().expect("payload"),
        serde_json::json!({"providerID": "opencode-go", "id": "glm-5.3"})
    );
    assert!(ModelRefDto::parse("glm-5.3").is_none());
    assert!(ModelRefDto::parse("opencode-go/").is_none());
    assert!(ModelRefDto::parse("/glm-5.3").is_none());
    assert!(ModelRefDto::parse("  ").is_none());
}

#[tokio::test]
async fn session_payload_never_contains_null_model_or_variant() {
    use std::sync::{Arc, Mutex};

    let bodies = Arc::new(Mutex::new(Vec::<Value>::new()));
    let capture = bodies.clone();
    let app = Router::new().route(
        "/api/session",
        post(move |Json(body): Json<Value>| {
            let capture = capture.clone();
            async move {
                capture.lock().expect("bodies").push(body);
                Json(serde_json::json!({ "data": { "id": "ses-1" } }))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let client = RestClient::new_with_url(&format!("http://{address}"), "secret".into());

    client
        .create_session(
            "/tmp/ws",
            "Test",
            "build",
            Some(ModelRefDto::parse("provider/model").expect("model")),
        )
        .await
        .expect("plain model");
    client
        .create_session("/tmp/ws", "Test", "build", None)
        .await
        .expect("no model");

    let bodies = bodies.lock().expect("bodies");
    assert_eq!(
        bodies[0]["model"],
        serde_json::json!({ "providerID": "provider", "id": "model" })
    );
    assert!(bodies[0]["model"].get("variant").is_none());
    assert!(bodies[1].get("model").is_none());

    handle.abort();
}

#[tokio::test]
async fn reports_transport_errors() {
    let client = RestClient::new_with_url("http://127.0.0.1:1", "secret".into());
    assert!(client.info().await.is_err());
}

#[tokio::test]
async fn contract_rejects_unauthorized_requests() {
    let (url, handle) = mock_server().await;
    let client = RestClient::new_with_url(&url, "wrong".into());
    let error = client.info().await.expect_err("unauthorized");
    assert!(error.to_string().contains("401"), "{error}");
    handle.abort();
}
