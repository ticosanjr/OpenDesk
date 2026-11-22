use super::*;
use axum::{
    Json, Router,
    response::IntoResponse,
    routing::{get, post},
};
use serde_json::Value;
use tokio_util::sync::CancellationToken;

async fn fake_opencode() -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route(
            "/api/session",
            post(|Json(_body): Json<Value>| async {
                Json(serde_json::json!({ "data": { "id": "ses-1" } }))
            }),
        )
        .route(
            "/api/event",
            get(|| async {
                (
                    [("content-type", "text/event-stream")],
                    "data: {\"type\":\"session.created\",\"data\":{\"sessionID\":\"ses-1\"}}\n\n",
                )
            }),
        )
        .route(
            "/api/session/{id}/prompt",
            post(|| async { axum::http::StatusCode::NO_CONTENT }),
        )
        .route(
            "/api/experimental/session/{id}/wait",
            post(|| async { axum::http::StatusCode::NO_CONTENT }),
        )
        .route(
            "/api/session/{id}/message",
            get(|| async {
                Json(serde_json::json!({
                    "data": [{
                        "type": "assistant",
                        "tokens": { "input": 10, "output": 2 },
                        "content": [{ "type": "text", "text": "OK" }]
                    }]
                }))
            }),
        )
        .route(
            "/api/session/{id}/diff",
            get(|| async { Json(serde_json::json!({ "data": [] })) }),
        )
        .fallback(|| async { axum::http::StatusCode::NOT_FOUND.into_response() });

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
async fn stream_close_falls_back_to_wait_and_messages() {
    let (url, handle) = fake_opencode().await;
    let rest = RestClient::new_with_url(&url, "secret".into());
    let temp = tempfile::TempDir::new().expect("temp");
    let pool = ::opendesk_storage::init(&temp.path().join("runner.db"))
        .await
        .expect("db");
    let log = temp.path().join("run.jsonl");
    let events = EventBus::new(16);
    let outcome = execute_run(
        rest,
        RunRequest {
            notifier: None,
            pool: &pool,
            secrets: &SecretStore::with_dir(temp.path().join("secrets")),
            workspace_id: "ws-1",
            run_id: "run-1",
            task_id: None,
            session_id: None,
            workspace_root: temp.path(),
            agent: "build",
            model: "deepseek/deepseek-flash",
            phase: "build",
            title: "Test",
            prompt: "oi",
            log_path: &log,
            cancel: CancellationToken::new(),
        },
        &events,
    )
    .await;
    assert_eq!(outcome.status, RunStatus::Success);
    assert_eq!(outcome.summary.as_deref(), Some("OK"));
    handle.abort();
}

#[tokio::test]
async fn stream_client_has_no_total_timeout() {
    // A 120s total timeout on the streaming client was the root cause of
    // long builds being marked failed after the artifact was written.
    let client = RestClient::new_with_url("http://127.0.0.1:1", "secret".into());
    let started = std::time::Instant::now();
    let result = client.event_response().await;
    assert!(result.is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
}

#[test]
fn chat_turn_signals_detects_tools_and_created_files() {
    let value = serde_json::json!({
        "data": [
            {
                "type": "assistant",
                "time": { "created": 2000 },
                "content": [
                    {
                        "type": "tool",
                        "name": "write",
                        "state": {
                            "status": "completed",
                            "input": { "path": "output/a.txt", "content": "oi" }
                        }
                    },
                    {
                        "type": "tool",
                        "name": "chat_send_file",
                        "state": {
                            "status": "completed",
                            "input": { "path": "output/b.pdf" }
                        }
                    }
                ]
            },
            {
                "type": "assistant",
                "time": { "created": 500 },
                "content": [{ "type": "tool", "name": "read", "state": { "status": "completed" } }]
            }
        ]
    });
    let (used_tools, writes) = chat_turn_signals(&value, 1000);
    assert!(used_tools);
    assert_eq!(writes.len(), 2);
    assert!(writes.iter().any(|write| write.path == "output/a.txt"));
    assert!(writes.iter().any(|write| write.path == "output/b.pdf"));
}
