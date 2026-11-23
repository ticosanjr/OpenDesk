use super::*;

#[test]
fn parses_model_refs_with_variants() {
    let model = parse_model_ref("opencode-go/glm-5.3#high").expect("model");
    assert_eq!(model.provider_id, "opencode-go");
    assert_eq!(model.id, "glm-5.3");
    assert_eq!(model.variant.as_deref(), Some("high"));
    assert_eq!(
        parse_model_ref("opencode-go/glm-5.3")
            .expect("plain")
            .variant,
        None
    );
    assert_eq!(
        parse_model_ref("opencode-go/glm-5.3# ")
            .expect("empty")
            .variant,
        None
    );
    assert!(parse_model_ref("no-provider").is_none());
}

#[test]
fn detects_only_missing_session_errors() {
    assert!(is_missing_session_error(
        "OpenCode prompt failed (404 Not Found): {\"error\":\"session not found\"}"
    ));
    assert!(is_missing_session_error("unknown session abc"));
    assert!(!is_missing_session_error(
        "OpenCode prompt failed (500): timeout"
    ));
    assert!(!is_missing_session_error("could not send OpenCode prompt"));
}

fn turn_fixture() -> serde_json::Value {
    serde_json::json!({
        "data": [
            {
                "id": "old",
                "type": "assistant",
                "time": { "created": 500, "completed": 600 },
                "cost": 99.0,
                "tokens": { "input": 1, "output": 1 },
                "content": [{ "type": "text", "text": "Olá! Sou o agente." }]
            },
            {
                "id": "new",
                "type": "assistant",
                "time": { "created": 2000, "completed": 2100 },
                "cost": 0.02,
                "tokens": { "input": 10, "output": 20 },
                "content": [
                    { "type": "reasoning", "text": "pensando" },
                    { "type": "tool", "name": "read" },
                    { "type": "text", "text": "Meu poema novo" }
                ]
            },
            { "id": "idle", "type": "idle", "time": { "created": 2200 } }
        ]
    })
}

#[test]
fn parses_only_the_current_turn_and_ignores_the_old_answer() {
    let value = turn_fixture();
    assert!(turn_has_idle(&value, 1000));
    assert!(turn_has_text(&value, 1000));
    assert!(!turn_has_text(&value, 2300));
    assert!(!turn_has_idle(&value, 2300));
    let (text, usage) = parse_assistant_since(&value, 1000);
    assert_eq!(text.as_deref(), Some("Meu poema novo"));
    assert_eq!(usage.tokens_input, 10);
    assert_eq!(usage.tokens_output, 20);
    assert!((usage.estimated_cost_usd - 0.02).abs() < f64::EPSILON);

    let (old, usage) = parse_assistant_since(&value, 3000);
    assert!(old.is_none(), "never fall back to an older answer");
    assert_eq!(usage.tokens_input, 0);
}

#[tokio::test]
async fn waits_for_a_new_turn_instead_of_returning_the_previous_answer() {
    use axum::{Json, Router, extract::State, routing::get};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    async fn messages(State(calls): State<Arc<AtomicUsize>>) -> Json<serde_json::Value> {
        let call = calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            Json(serde_json::json!({
                "data": [{
                    "id": "old",
                    "type": "assistant",
                    "time": { "created": 500, "completed": 600 },
                    "content": [{ "type": "text", "text": "Olá! Sou o agente." }]
                }]
            }))
        } else {
            Json(turn_fixture())
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route("/api/session/{id}/message", get(messages))
        .with_state(calls);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let client = crate::opencode_client::rest_client::RestClient::new_with_url(
        &format!("http://{address}"),
        "secret".into(),
    );
    let value = wait_for_turn(
        &client,
        "ses-1",
        1000,
        &CancellationToken::new(),
        None,
        Duration::from_secs(60),
    )
    .await
    .expect("turn");
    let (text, _) = parse_assistant_since(&value, 1000);
    assert_eq!(text.as_deref(), Some("Meu poema novo"));
    handle.abort();
}

async fn turn_server() -> (
    crate::opencode_client::rest_client::RestClient,
    tokio::task::JoinHandle<()>,
) {
    use axum::{Json, Router, routing::get};

    async fn messages() -> Json<serde_json::Value> {
        Json(turn_fixture())
    }

    let app = Router::new().route("/api/session/{id}/message", get(messages));
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let client = crate::opencode_client::rest_client::RestClient::new_with_url(
        &format!("http://{address}"),
        "secret".into(),
    );
    (client, handle)
}

#[tokio::test]
async fn terminal_session_event_wakes_the_turn_immediately() {
    let (client, handle) = turn_server().await;
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    tx.send(serde_json::json!({
        "type": "session.idle",
        "data": { "sessionID": "ses-1" }
    }))
    .await
    .expect("event");
    let started = std::time::Instant::now();
    let cancel = CancellationToken::new();
    let value = wait_for_turn(
        &client,
        "ses-1",
        1000,
        &cancel,
        Some(rx),
        Duration::from_secs(60),
    )
    .await
    .expect("turn");
    assert!(started.elapsed() < std::time::Duration::from_millis(500));
    let (text, _) = parse_assistant_since(&value, 1000);
    assert_eq!(text.as_deref(), Some("Meu poema novo"));
    handle.abort();
}

#[tokio::test]
async fn times_out_without_a_final_turn() {
    use axum::{Json, Router, routing::get};

    async fn old() -> Json<serde_json::Value> {
        Json(serde_json::json!({
            "data": [{
                "id": "old",
                "type": "assistant",
                "time": { "created": 500 },
                "content": [{ "type": "text", "text": "resposta antiga" }]
            }]
        }))
    }

    let app = Router::new().route("/api/session/{id}/message", get(old));
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let client = crate::opencode_client::rest_client::RestClient::new_with_url(
        &format!("http://{address}"),
        "secret".into(),
    );
    let cancel = CancellationToken::new();
    let error = wait_for_turn(
        &client,
        "ses-1",
        1000,
        &cancel,
        None,
        Duration::from_millis(300),
    )
    .await
    .expect_err("timeout");
    assert!(error.contains("excedeu"), "{error}");
    handle.abort();
}

#[tokio::test]
async fn ignores_terminal_events_from_another_session() {
    let (client, handle) = turn_server().await;
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    tx.send(serde_json::json!({
        "type": "session.idle",
        "data": { "sessionID": "ses-other" }
    }))
    .await
    .expect("other event");
    let cancel = CancellationToken::new();
    let wait = wait_for_turn(
        &client,
        "ses-1",
        1000,
        &cancel,
        Some(rx),
        Duration::from_secs(60),
    );
    tokio::pin!(wait);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(300), &mut wait)
            .await
            .is_err(),
        "another session must not wake this turn"
    );
    tx.send(serde_json::json!({
        "type": "session.idle",
        "data": { "sessionID": "ses-1" }
    }))
    .await
    .expect("own event");
    let value = tokio::time::timeout(std::time::Duration::from_secs(1), wait)
        .await
        .expect("timeout")
        .expect("turn");
    let (text, _) = parse_assistant_since(&value, 1000);
    assert_eq!(text.as_deref(), Some("Meu poema novo"));
    handle.abort();
}

#[tokio::test]
async fn does_not_stop_on_a_tool_only_message() {
    use axum::{Json, Router, extract::State, routing::get};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    async fn messages(State(calls): State<Arc<AtomicUsize>>) -> Json<serde_json::Value> {
        let call = calls.fetch_add(1, Ordering::SeqCst);
        if call < 3 {
            // The agent is still working: an assistant message exists, but
            // it only carries a tool call and there is no idle marker.
            Json(serde_json::json!({
                "data": [{
                    "id": "tool-step",
                    "type": "assistant",
                    "time": { "created": 2000 },
                    "content": [{
                        "type": "tool",
                        "name": "execute",
                        "state": { "status": "completed" }
                    }]
                }]
            }))
        } else {
            Json(turn_fixture())
        }
    }

    let calls = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route("/api/session/{id}/message", get(messages))
        .with_state(calls);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let client = crate::opencode_client::rest_client::RestClient::new_with_url(
        &format!("http://{address}"),
        "secret".into(),
    );
    let value = wait_for_turn(
        &client,
        "ses-1",
        1000,
        &CancellationToken::new(),
        None,
        Duration::from_secs(60),
    )
    .await
    .expect("turn with tools");
    let (text, _) = parse_assistant_since(&value, 1000);
    assert_eq!(text.as_deref(), Some("Meu poema novo"));
    handle.abort();
}
