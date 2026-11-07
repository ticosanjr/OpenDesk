use crate::{
    app_state::AppState,
    db::{self, permissions},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
    io_bus::{NormalizedTaskEvent, source_config},
};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Default, Deserialize)]
pub struct TelegramConfig {
    #[serde(default)]
    pub token_ref: Option<String>,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub allowed_chats: Vec<String>,
}
#[derive(Debug, Default, Deserialize)]
pub struct MatrixConfig {
    pub homeserver: String,
    #[serde(default)]
    pub access_token_ref: Option<String>,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub rooms: Vec<String>,
}

pub async fn run(
    state: Arc<AppState>,
    source: db::io_bus::InboundSource,
    cancel: CancellationToken,
) -> AppResult<()> {
    match source.source_type.as_str() {
        "TELEGRAM_BOT" => run_telegram(state, source, cancel).await,
        "MATRIX_BOT" => run_matrix(state, source, cancel).await,
        "MATTERMOST_BOT" => Ok(()),
        other => Err(AppError::BadRequest(format!(
            "chat provider não suportado: {other}"
        ))),
    }
}

async fn run_telegram(
    state: Arc<AppState>,
    source: db::io_bus::InboundSource,
    cancel: CancellationToken,
) -> AppResult<()> {
    let config: TelegramConfig = source_config(&source)?;
    let token = secret(&state, config.token_ref.as_deref(), config.token.as_deref())?;
    let client = crate::io_bus::http_client(40);
    let mut offset = 0i64;
    loop {
        if cancel.is_cancelled() {
            break;
        }
        let response = client
            .get(format!("https://api.telegram.org/bot{token}/getUpdates"))
            .query(&[
                ("timeout", "25".to_string()),
                ("offset", offset.to_string()),
                (
                    "allowed_updates",
                    "[\"message\",\"callback_query\"]".to_string(),
                ),
            ])
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                tracing::warn!(error = %error, "Telegram poll failed");
                sleep_or_cancel(&cancel, 5).await;
                continue;
            }
        };
        let value: serde_json::Value = response.json().await.unwrap_or_default();
        if let Some(updates) = value.get("result").and_then(|value| value.as_array()) {
            for update in updates {
                if let Some(id) = update.get("update_id").and_then(|value| value.as_i64()) {
                    offset = offset.max(id + 1);
                }
                handle_telegram_update(&state, &source, &client, &token, update, &config).await;
            }
        }
        if value
            .get("result")
            .and_then(|value| value.as_array())
            .map(|items| items.is_empty())
            .unwrap_or(true)
        {
            sleep_or_cancel(&cancel, 1).await;
        }
    }
    Ok(())
}

async fn handle_telegram_update(
    state: &Arc<AppState>,
    source: &db::io_bus::InboundSource,
    client: &reqwest::Client,
    token: &str,
    update: &serde_json::Value,
    config: &TelegramConfig,
) {
    if let Some(callback) = update.get("callback_query") {
        let data = callback
            .get("data")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let callback_id = callback
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        let decision = if data.ends_with(":once") {
            Some("once")
        } else if data.ends_with(":always") {
            Some("always")
        } else if data.ends_with(":reject") {
            Some("reject")
        } else {
            None
        };
        if let Some(decision) = decision
            && let Some(request_id) = data
                .strip_prefix("perm:")
                .and_then(|value| value.rsplit_once(':'))
                .map(|(request, _)| request)
            && let Ok(request) = permissions::decide(&state.db, request_id, decision).await
        {
            state.events.publish(OpenDeskWsEvent::PermissionDecided {
                request_id: request.request_id,
                decision: decision.to_string(),
            });
        }
        let _ = client
            .post(format!(
                "https://api.telegram.org/bot{token}/answerCallbackQuery"
            ))
            .json(&serde_json::json!({ "callback_query_id": callback_id, "text": "OpenDesk" }))
            .send()
            .await;
        return;
    }
    let Some(message) = update.get("message") else {
        return;
    };
    let chat_id = message
        .pointer("/chat/id")
        .and_then(|value| value.as_i64())
        .map(|value| value.to_string())
        .unwrap_or_default();
    if chat_id.is_empty()
        || (!config.allowed_chats.is_empty() && !config.allowed_chats.contains(&chat_id))
    {
        return;
    }
    let text = message
        .get("text")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim();
    if text.is_empty() {
        return;
    }
    if let Some(rest) = text.strip_prefix("!aprovar ") {
        if let Ok(Some(request)) = permissions::decide_prefix(&state.db, rest, "once").await {
            state.events.publish(OpenDeskWsEvent::PermissionDecided {
                request_id: request.request_id,
                decision: "once".into(),
            });
            let _ =
                crate::io_bus::chat::send_telegram(token, &chat_id, "Permissão aprovada.", None)
                    .await;
        }
        return;
    }
    if let Some(rest) = text.strip_prefix("!negar ") {
        if let Ok(Some(request)) = permissions::decide_prefix(&state.db, rest, "reject").await {
            state.events.publish(OpenDeskWsEvent::PermissionDecided {
                request_id: request.request_id,
                decision: "reject".into(),
            });
            let _ = crate::io_bus::chat::send_telegram(token, &chat_id, "Permissão negada.", None)
                .await;
        }
        return;
    }
    let external_id = message
        .get("message_id")
        .and_then(|value| value.as_i64())
        .map(|value| value.to_string())
        .unwrap_or_else(|| text.to_string());
    let event = NormalizedTaskEvent {
        source_id: source.id.clone(),
        external_id,
        title: text
            .lines()
            .next()
            .unwrap_or("Telegram")
            .chars()
            .take(100)
            .collect(),
        body_markdown: text.to_string(),
        priority: None,
        tags: vec!["telegram".into()],
        attachments: vec![],
        metadata: serde_json::json!({ "chat_id": chat_id }),
    };
    if let Err(error) = crate::io_bus::ingest(state, source, event).await {
        tracing::warn!(error = %error, "Telegram ingest failed");
    }
    let _ = crate::io_bus::chat::send_telegram(token, &chat_id, "Tarefa criada no OpenDesk.", None)
        .await;
}

async fn run_matrix(
    state: Arc<AppState>,
    source: db::io_bus::InboundSource,
    cancel: CancellationToken,
) -> AppResult<()> {
    let config: MatrixConfig = source_config(&source)?;
    let token = secret(
        &state,
        config.access_token_ref.as_deref(),
        config.access_token.as_deref(),
    )?;
    let client = crate::io_bus::http_client(40);
    let mut since: Option<String> = None;
    loop {
        if cancel.is_cancelled() {
            break;
        }
        let mut request = client
            .get(format!(
                "{}/_matrix/client/v3/sync?timeout=30000",
                config.homeserver.trim_end_matches('/')
            ))
            .bearer_auth(&token);
        if let Some(since) = &since {
            request = request.query(&[("since", since)]);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) => {
                tracing::warn!(error = %error, "Matrix sync failed");
                sleep_or_cancel(&cancel, 5).await;
                continue;
            }
        };
        let value: serde_json::Value = response.json().await.unwrap_or_default();
        since = value
            .get("next_batch")
            .and_then(|value| value.as_str())
            .map(str::to_string)
            .or(since);
        if let Some(rooms) = value
            .pointer("/rooms/join")
            .and_then(|value| value.as_object())
        {
            for (room_id, room) in rooms {
                if !config.rooms.is_empty() && !config.rooms.iter().any(|item| item == room_id) {
                    continue;
                }
                let Some(events) = room
                    .pointer("/timeline/events")
                    .and_then(|value| value.as_array())
                else {
                    continue;
                };
                for event in events {
                    if event.get("type").and_then(|value| value.as_str()) != Some("m.room.message")
                    {
                        continue;
                    }
                    let text = event
                        .pointer("/content/body")
                        .and_then(|value| value.as_str())
                        .unwrap_or_default();
                    if text.is_empty() {
                        continue;
                    }
                    if let Some(rest) = text.strip_prefix("!aprovar ") {
                        let _ = permissions::decide_prefix(&state.db, rest, "once").await;
                        continue;
                    }
                    if let Some(rest) = text.strip_prefix("!negar ") {
                        let _ = permissions::decide_prefix(&state.db, rest, "reject").await;
                        continue;
                    }
                    let event_id = event
                        .get("event_id")
                        .and_then(|value| value.as_str())
                        .unwrap_or(text);
                    let event = NormalizedTaskEvent {
                        source_id: source.id.clone(),
                        external_id: event_id.to_string(),
                        title: text
                            .lines()
                            .next()
                            .unwrap_or("Matrix")
                            .chars()
                            .take(100)
                            .collect(),
                        body_markdown: text.to_string(),
                        priority: None,
                        tags: vec!["matrix".into()],
                        attachments: vec![],
                        metadata: serde_json::json!({ "room_id": room_id }),
                    };
                    let _ = crate::io_bus::ingest(&state, &source, event).await;
                }
            }
        }
    }
    Ok(())
}

fn secret(
    state: &Arc<AppState>,
    reference: Option<&str>,
    plain: Option<&str>,
) -> AppResult<String> {
    crate::io_bus::secret(&state.paths, reference, plain)
}

async fn sleep_or_cancel(cancel: &CancellationToken, seconds: u64) {
    tokio::select! {
        _ = cancel.cancelled() => {}
        _ = tokio::time::sleep(Duration::from_secs(seconds)) => {}
    }
}
