use crate::error::{AppError, AppResult};

/// Shared Telegram send primitive used by outbound sinks and inbound bots.
pub async fn send_telegram(
    token: &str,
    chat_id: &str,
    text: &str,
    parse_mode: Option<&str>,
) -> AppResult<()> {
    let mut body = serde_json::json!({
        "chat_id": chat_id,
        "text": text,
        "disable_web_page_preview": true
    });
    if let Some(mode) = parse_mode {
        body["parse_mode"] = serde_json::Value::String(mode.to_string());
    }
    let response = crate::io_bus::http_client(30)
        .post(format!("https://api.telegram.org/bot{token}/sendMessage"))
        .json(&body)
        .send()
        .await
        .map_err(|error| AppError::Daemon(format!("Telegram: {error}")))?;
    if !response.status().is_success() {
        return Err(AppError::Daemon(format!(
            "Telegram respondeu {}",
            response.status()
        )));
    }
    Ok(())
}

/// Shared Matrix message send primitive.
pub async fn send_matrix(
    homeserver: &str,
    token: &str,
    room_id: &str,
    text: &str,
) -> AppResult<()> {
    let txn = uuid::Uuid::now_v7();
    let response = crate::io_bus::http_client(30)
        .put(format!(
            "{}/_matrix/client/v3/rooms/{room_id}/send/m.room.message/{txn}",
            homeserver.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .json(&serde_json::json!({ "msgtype": "m.text", "body": text }))
        .send()
        .await
        .map_err(|error| AppError::Daemon(format!("Matrix: {error}")))?;
    if !response.status().is_success() {
        return Err(AppError::Daemon(format!(
            "Matrix respondeu {}",
            response.status()
        )));
    }
    Ok(())
}
