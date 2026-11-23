use crate::error::{AppError, AppResult};

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
