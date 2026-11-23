//! Discord REST client.

use crate::error::{AppError, AppResult};
use serde_json::{Value, json};

fn api_base() -> String {
    std::env::var("OPENDESK_DISCORD_API").unwrap_or_else(|_| "https://discord.com/api/v10".into())
}

pub async fn send_text(token: &str, channel_id: &str, text: &str) -> AppResult<()> {
    send_message(&api_base(), token, channel_id, json!({ "content": text })).await
}

pub async fn send_text_reply(
    token: &str,
    channel_id: &str,
    text: &str,
    message_id: &str,
) -> AppResult<()> {
    send_message(
        &api_base(),
        token,
        channel_id,
        json!({
            "content": text,
            "message_reference": { "message_id": message_id },
        }),
    )
    .await
}

pub async fn send_hitl(
    token: &str,
    channel_id: &str,
    title: &str,
    body: &str,
    actions: &[(String, String)],
) -> AppResult<()> {
    let mut components = Vec::new();
    if !actions.is_empty() {
        let buttons = actions
            .iter()
            .take(5)
            .map(|(id, label)| {
                json!({
                    "type": 2,
                    "style": 1,
                    "label": label,
                    "custom_id": id,
                })
            })
            .collect::<Vec<_>>();
        components.push(json!({ "type": 1, "components": buttons }));
    }
    send_message(
        &api_base(),
        token,
        channel_id,
        json!({
            "content": format!("**{title}**\n{body}"),
            "components": components,
        }),
    )
    .await
}

async fn send_message(api: &str, token: &str, channel_id: &str, body: Value) -> AppResult<()> {
    let response = crate::http_client()
        .post(format!(
            "{}/channels/{channel_id}/messages",
            api.trim_end_matches('/')
        ))
        .header("authorization", format!("Bot {token}"))
        .json(&body)
        .send()
        .await
        .map_err(|error| AppError::Internal(format!("Discord API inacessível: {error}")))?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!(
            "Discord respondeu {status}: {}",
            text.chars().take(300).collect::<String>()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn posts_messages_and_maps_http_errors() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut buffer = vec![0u8; 2048];
            let _ = socket.read(&mut buffer).await;
            let _ = socket
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok")
                .await;
            let (mut socket, _) = listener.accept().await.expect("accept 2");
            let _ = socket.read(&mut buffer).await;
            let _ = socket
                .write_all(b"HTTP/1.1 400 Bad Request\r\ncontent-length: 3\r\nconnection: close\r\n\r\nbad")
                .await;
        });
        let api = format!("http://{address}");
        send_message(&api, "token", "channel", json!({ "content": "oi" }))
            .await
            .expect("send");
        assert!(
            send_message(&api, "token", "channel", json!({}))
                .await
                .is_err()
        );
    }
}
