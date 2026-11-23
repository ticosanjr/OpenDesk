//! Slack Web API and Socket Mode helpers.

use crate::error::{AppError, AppResult};
use serde_json::{Value, json};

fn api_base() -> String {
    std::env::var("OPENDESK_SLACK_API").unwrap_or_else(|_| "https://slack.com/api".into())
}

pub async fn connections_open(app_token: &str) -> AppResult<String> {
    connections_open_at(&api_base(), app_token).await
}

pub async fn connections_open_at(base: &str, app_token: &str) -> AppResult<String> {
    let response = crate::http_client()
        .post(format!(
            "{}/apps.connections.open",
            base.trim_end_matches('/')
        ))
        .bearer_auth(app_token)
        .send()
        .await
        .map_err(|error| AppError::Internal(format!("Slack inacessível: {error}")))?;
    let value: Value = response
        .json()
        .await
        .map_err(|error| AppError::Internal(format!("Slack JSON: {error}")))?;
    if value.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(AppError::BadRequest(format!(
            "Slack connections.open falhou: {}",
            value.get("error").and_then(Value::as_str).unwrap_or("erro")
        )));
    }
    value
        .get("url")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| AppError::Internal("Slack não devolveu URL Socket Mode".into()))
}

pub async fn send_text(bot_token: &str, channel: &str, text: &str) -> AppResult<()> {
    post_message(bot_token, channel, json!({ "text": text })).await
}

pub async fn send_text_in_thread(
    bot_token: &str,
    channel: &str,
    text: &str,
    thread_ts: &str,
) -> AppResult<()> {
    post_message(
        bot_token,
        channel,
        json!({ "text": text, "thread_ts": thread_ts }),
    )
    .await
}

pub async fn send_hitl(
    bot_token: &str,
    channel: &str,
    title: &str,
    body: &str,
    actions: &[(String, String)],
) -> AppResult<()> {
    let elements = actions
        .iter()
        .take(5)
        .map(|(id, label)| {
            json!({
                "type": "button",
                "action_id": id,
                "text": { "type": "plain_text", "text": label },
                "value": id,
            })
        })
        .collect::<Vec<_>>();
    let blocks = json!([
        { "type": "section", "text": { "type": "mrkdwn", "text": format!("*{title}*\n{body}") } },
        { "type": "actions", "elements": elements }
    ]);
    post_message(
        bot_token,
        channel,
        json!({ "text": format!("{title}: {body}"), "blocks": blocks }),
    )
    .await
}

async fn post_message(bot_token: &str, channel: &str, body: Value) -> AppResult<()> {
    post_message_at(&api_base(), bot_token, channel, body).await
}

async fn post_message_at(
    base: &str,
    bot_token: &str,
    channel: &str,
    mut body: Value,
) -> AppResult<()> {
    if let Some(object) = body.as_object_mut() {
        object.insert("channel".into(), Value::String(channel.to_string()));
    }
    let response = crate::http_client()
        .post(format!("{}/chat.postMessage", base.trim_end_matches('/')))
        .bearer_auth(bot_token)
        .json(&body)
        .send()
        .await
        .map_err(|error| AppError::Internal(format!("Slack inacessível: {error}")))?;
    let value: Value = response
        .json()
        .await
        .map_err(|error| AppError::Internal(format!("Slack JSON: {error}")))?;
    if value.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(AppError::Internal(format!(
            "Slack postMessage falhou: {}",
            value.get("error").and_then(Value::as_str).unwrap_or("erro")
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn opens_socket_and_posts_messages() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            for body in [
                r#"{"ok":true,"url":"wss://example.invalid/socket"}"#,
                r#"{"ok":true}"#,
            ] {
                let (mut socket, _) = listener.accept().await.expect("accept");
                let mut buffer = vec![0u8; 2048];
                let _ = socket.read(&mut buffer).await;
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });
        let base = format!("http://{address}");
        assert!(
            connections_open_at(&base, "app")
                .await
                .expect("open")
                .contains("example.invalid")
        );
        post_message_at(&base, "bot", "C1", json!({ "text": "oi" }))
            .await
            .expect("post");
    }
}
