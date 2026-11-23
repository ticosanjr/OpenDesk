//! Telegram Bot API HTTP client and outbound message helpers.

use crate::error::{AppError, AppResult};
use opendesk_engine::media::mime_for_path;
use opendesk_storage::secrets::SecretStore;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use std::path::Path;
use std::time::Duration;

use super::config::{load, resolve_recipients, token_for_parts};
use super::parse::{chunks, parse_updates, truncate};
use super::types::{BotInfo, TelegramUpdate};

const MAX_MESSAGE_CHARS: usize = 3800;
/// Bot API download limit. Larger files require a local Bot API server.
pub const MAX_FILE_BYTES: usize = 20 * 1024 * 1024;

pub async fn send_text(token: &str, chat_id: &str, text: &str) -> AppResult<()> {
    send_text_with_parse_mode(token, chat_id, text, None).await
}

/// Sends text with an optional Bot API `parse_mode` (for example `HTML`).
/// Used by the I/O bus, which formats artifacts as HTML.
pub async fn send_text_with_parse_mode(
    token: &str,
    chat_id: &str,
    text: &str,
    parse_mode: Option<&str>,
) -> AppResult<()> {
    let text = if text.trim().is_empty() {
        "(sem resposta)"
    } else {
        text
    };
    if let Some(mode) = parse_mode {
        // Parse-mode payloads are HTML/Markdown and may not survive chunking;
        // preserve the historical single-message behavior for the I/O bus.
        return send_chunk_with_mode(token, chat_id, text, None, None, Some(mode)).await;
    }
    for chunk in chunks(text, MAX_MESSAGE_CHARS) {
        send_chunk_with_mode(token, chat_id, &chunk, None, None, None).await?;
    }
    Ok(())
}

pub async fn send_reply(token: &str, chat_id: &str, message_id: i64, text: &str) -> AppResult<()> {
    send_text_with_markup(token, chat_id, text, Some(message_id), None).await
}

pub async fn send_text_with_markup(
    token: &str,
    chat_id: &str,
    text: &str,
    reply_to: Option<i64>,
    markup: Option<&Value>,
) -> AppResult<()> {
    let text = if text.trim().is_empty() {
        "(sem resposta)"
    } else {
        text
    };
    for (index, chunk) in chunks(text, MAX_MESSAGE_CHARS).into_iter().enumerate() {
        send_chunk(
            token,
            chat_id,
            &chunk,
            if index == 0 { reply_to } else { None },
            if index == 0 { markup } else { None },
        )
        .await?;
    }
    Ok(())
}

async fn send_chunk(
    token: &str,
    chat_id: &str,
    text: &str,
    reply_to: Option<i64>,
    markup: Option<&Value>,
) -> AppResult<()> {
    send_chunk_with_mode(token, chat_id, text, reply_to, markup, None).await
}

async fn send_chunk_with_mode(
    token: &str,
    chat_id: &str,
    text: &str,
    reply_to: Option<i64>,
    markup: Option<&Value>,
    parse_mode: Option<&str>,
) -> AppResult<()> {
    let mut payload = json!({
        "chat_id": chat_id,
        "text": text,
        "disable_web_page_preview": true,
    });
    if let Some(message_id) = reply_to {
        payload["reply_parameters"] = json!({
            "message_id": message_id,
            "allow_sending_without_reply": true,
        });
    }
    if let Some(markup) = markup {
        payload["reply_markup"] = markup.clone();
    }
    if let Some(mode) = parse_mode {
        payload["parse_mode"] = Value::String(mode.to_string());
    }
    let response = client()
        .post(api_url(token, "sendMessage"))
        .json(&payload)
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha de rede ao enviar mensagem".into()))?;
    parse_response("sendMessage", response).await.map(|_| ())
}

/// Send to every allowlist target that already has a numeric chat ID.
pub async fn send_to_allowlist_parts(
    pool: &SqlitePool,
    secrets: &SecretStore,
    text: &str,
) -> AppResult<usize> {
    let config = load(pool).await?;
    if !config.enabled {
        return Err(AppError::BadRequest("Telegram não está habilitado".into()));
    }
    let token = token_for_parts(secrets)?;
    let targets = resolve_recipients(pool, &config.allowlist).await;
    if targets.is_empty() {
        return Err(AppError::BadRequest(
            "a allowlist do Telegram não tem chats conhecidos; envie /start para o bot primeiro"
                .into(),
        ));
    }
    for target in &targets {
        send_text(&token, target, text).await?;
    }
    Ok(targets.len())
}

/// Uploads a local file back to the chat. Used by completion watchers and the
/// outbound Telegram sink.
pub async fn send_document(
    token: &str,
    chat_id: &str,
    path: &Path,
    caption: Option<&str>,
) -> AppResult<()> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|error| AppError::BadRequest(format!("arquivo indisponível: {error}")))?;
    if bytes.len() > 49 * 1024 * 1024 {
        return Err(AppError::BadRequest(
            "arquivo excede o limite de 50 MB do Telegram".into(),
        ));
    }
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact")
        .to_string();
    let caption = caption
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| truncate(value, 900));
    let (boundary, body) = multipart_document(
        chat_id,
        &file_name,
        mime_for_path(path),
        bytes,
        caption.as_deref(),
    );
    let response = client()
        .post(api_url(token, "sendDocument"))
        .header(
            reqwest::header::CONTENT_TYPE,
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(body)
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha de rede ao enviar documento".into()))?;
    parse_response("sendDocument", response).await.map(|_| ())
}

/// Builds the `multipart/form-data` body for `sendDocument` without the
/// optional `reqwest/multipart` feature, which this crate does not enable.
fn multipart_document(
    chat_id: &str,
    file_name: &str,
    mime: &str,
    bytes: Vec<u8>,
    caption: Option<&str>,
) -> (String, Vec<u8>) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let boundary = format!(
        "opendesk-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let file_name = file_name.replace('\\', "\\\\").replace('"', "\\\"");
    let mut body = Vec::with_capacity(bytes.len() + 512);
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"chat_id\"\r\n\r\n{chat_id}\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"document\"; filename=\"{file_name}\"\r\nContent-Type: {mime}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(&bytes);
    body.extend_from_slice(b"\r\n");
    if let Some(caption) = caption {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"caption\"\r\n\r\n{caption}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    (boundary, body)
}

pub async fn download_file(token: &str, file_id: &str, destination: &Path) -> AppResult<()> {
    let response = client()
        .post(api_url(token, "getFile"))
        .json(&json!({ "file_id": file_id }))
        .timeout(Duration::from_secs(20))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha de rede no getFile".into()))?;
    let value = parse_response("getFile", response).await?;
    if value
        .get("file_size")
        .and_then(Value::as_i64)
        .is_some_and(|size| size > MAX_FILE_BYTES as i64)
    {
        return Err(AppError::BadRequest(format!(
            "o arquivo excede o limite de {} MB do Bot API",
            MAX_FILE_BYTES / (1024 * 1024)
        )));
    }
    let file_path = value
        .get("file_path")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Daemon("Telegram getFile sem file_path".into()))?;
    let response = client()
        .get(format!(
            "https://api.telegram.org/file/bot{token}/{file_path}"
        ))
        .timeout(Duration::from_secs(120))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha de rede ao baixar o arquivo".into()))?;
    if !response.status().is_success() {
        return Err(AppError::Daemon(format!(
            "Telegram respondeu {} ao baixar o arquivo",
            response.status()
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| AppError::Daemon("Telegram: arquivo interrompido".into()))?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err(AppError::BadRequest(format!(
            "o arquivo excede o limite de {} MB do Bot API",
            MAX_FILE_BYTES / (1024 * 1024)
        )));
    }
    if let Some(parent) = destination.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            AppError::Internal(format!("could not create input folder: {error}"))
        })?;
    }
    tokio::fs::write(destination, &bytes)
        .await
        .map_err(|error| AppError::Internal(format!("could not save Telegram file: {error}")))?;
    Ok(())
}

pub async fn get_me(token: &str) -> AppResult<BotInfo> {
    let response = client()
        .get(api_url(token, "getMe"))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha de rede ao validar o token".into()))?;
    let value = parse_response("getMe", response).await?;
    serde_json::from_value(value).map_err(|error| {
        AppError::Daemon(format!("Telegram getMe retornou dados inválidos: {error}"))
    })
}

pub async fn delete_webhook(token: &str) -> AppResult<()> {
    let response = client()
        .post(api_url(token, "deleteWebhook"))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha de rede ao liberar o polling".into()))?;
    parse_response("deleteWebhook", response).await.map(|_| ())
}

pub async fn get_updates(token: &str, offset: i64) -> AppResult<Vec<TelegramUpdate>> {
    let value = get_updates_value(token, offset).await?;
    Ok(parse_updates(&value))
}

/// Raw form used by the I/O bus bridge, which keeps its own update handler.
pub async fn get_updates_value(token: &str, offset: i64) -> AppResult<Value> {
    let response = client()
        .post(api_url(token, "getUpdates"))
        .json(&json!({
            "offset": offset,
            "timeout": 25,
            "allowed_updates": ["message", "callback_query"],
        }))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha de rede no long polling".into()))?;
    parse_response("getUpdates", response).await
}

async fn parse_response(method: &str, response: reqwest::Response) -> AppResult<Value> {
    let status = response.status();
    let value: Value = response
        .json()
        .await
        .map_err(|_| AppError::Daemon(format!("Telegram {method}: resposta inválida")))?;
    if !status.is_success() || value.get("ok").and_then(Value::as_bool) != Some(true) {
        let description = value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("erro desconhecido");
        return Err(AppError::Daemon(format!(
            "Telegram {method} falhou ({status}): {description}"
        )));
    }
    Ok(value.get("result").cloned().unwrap_or(Value::Null))
}

fn api_url(token: &str, method: &str) -> String {
    format!("https://api.telegram.org/bot{token}/{method}")
}

fn client() -> reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .timeout(Duration::from_secs(45))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        })
        .clone()
}

pub async fn send_chat_action(token: &str, chat_id: &str, action: &str) -> AppResult<()> {
    let response = client()
        .post(api_url(token, "sendChatAction"))
        .json(&json!({ "chat_id": chat_id, "action": action }))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha de rede no typing".into()))?;
    parse_response("sendChatAction", response).await.map(|_| ())
}

pub async fn answer_callback_query(token: &str, callback_id: &str, text: &str) -> AppResult<()> {
    let response = client()
        .post(api_url(token, "answerCallbackQuery"))
        .json(&json!({ "callback_query_id": callback_id, "text": text }))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha ao responder callback".into()))?;
    parse_response("answerCallbackQuery", response)
        .await
        .map(|_| ())
}

pub async fn clear_buttons(token: &str, chat_id: &str, message_id: i64) -> AppResult<()> {
    let response = client()
        .post(api_url(token, "editMessageReplyMarkup"))
        .json(&json!({
            "chat_id": chat_id,
            "message_id": message_id,
            "reply_markup": { "inline_keyboard": [] },
        }))
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|_| AppError::Daemon("Telegram: falha ao limpar botões".into()))?;
    parse_response("editMessageReplyMarkup", response)
        .await
        .map(|_| ())
}
