//! Pure Telegram parsing and text formatting helpers.

use serde_json::Value;

use super::types::{IncomingCallback, IncomingTelegram, IncomingTelegramFile, TelegramUpdate};

pub fn parse_updates(value: &Value) -> Vec<TelegramUpdate> {
    let items = value
        .as_array()
        .or_else(|| value.get("result").and_then(Value::as_array));
    let Some(items) = items else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            if let Some(message) = parse_update(item) {
                Some(TelegramUpdate::Message(message))
            } else {
                parse_callback_update(item).map(TelegramUpdate::Callback)
            }
        })
        .collect()
}

pub fn parse_update(value: &Value) -> Option<IncomingTelegram> {
    let update_id = value.get("update_id")?.as_i64()?;
    let message = value.get("message")?;
    let from = message.get("from");
    if from
        .and_then(|from| from.get("is_bot"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let message_id = message.get("message_id")?.as_i64()?;
    let chat = message.get("chat")?;
    let chat_id = chat.get("id")?.as_i64()?.to_string();
    let username = from
        .and_then(|from| from.get("username"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let chat_title = chat
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_string);
    let text = message
        .get("text")
        .or_else(|| message.get("caption"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .unwrap_or_default()
        .to_string();
    let file = parse_message_file(message, message_id);
    if text.is_empty() && file.is_none() {
        return None;
    }
    Some(IncomingTelegram {
        update_id,
        message_id,
        chat_id,
        username,
        chat_title,
        text,
        file,
    })
}

fn parse_message_file(message: &Value, message_id: i64) -> Option<IncomingTelegramFile> {
    if let Some(document) = message.get("document") {
        return file_from(document, message_id, "document");
    }
    if let Some(photos) = message.get("photo").and_then(Value::as_array) {
        let best = photos.iter().max_by_key(|photo| {
            photo
                .get("file_size")
                .and_then(Value::as_i64)
                .unwrap_or_default()
        })?;
        return file_from(best, message_id, "photo");
    }
    for key in ["audio", "voice", "video"] {
        if let Some(file) = message.get(key) {
            return file_from(file, message_id, key);
        }
    }
    None
}

fn file_from(value: &Value, message_id: i64, kind: &str) -> Option<IncomingTelegramFile> {
    let file_id = value.get("file_id")?.as_str()?.to_string();
    let mime_type = value
        .get("mime_type")
        .and_then(Value::as_str)
        .map(str::to_string);
    let size = value.get("file_size").and_then(Value::as_i64);
    let file_name = value
        .get("file_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| default_file_name(kind, message_id, mime_type.as_deref()));
    Some(IncomingTelegramFile {
        file_id,
        file_name,
        mime_type,
        size,
    })
}

fn default_file_name(kind: &str, message_id: i64, mime: Option<&str>) -> String {
    let extension = match mime.unwrap_or_default() {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "audio/ogg" | "audio/opus" => "ogg",
        "audio/mpeg" => "mp3",
        "audio/mp4" => "m4a",
        "video/mp4" => "mp4",
        "application/pdf" => "pdf",
        _ => match kind {
            "photo" => "jpg",
            "voice" => "ogg",
            "video" => "mp4",
            "audio" => "mp3",
            _ => "bin",
        },
    };
    format!("{kind}-{message_id}.{extension}")
}

pub fn parse_callback_update(value: &Value) -> Option<IncomingCallback> {
    let update_id = value.get("update_id")?.as_i64()?;
    let callback = value.get("callback_query")?;
    let callback_id = callback.get("id")?.as_str()?.to_string();
    let from = callback.get("from");
    if from
        .and_then(|from| from.get("is_bot"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let message = callback.get("message")?;
    let message_id = message.get("message_id")?.as_i64()?;
    let chat = message.get("chat")?;
    let chat_id = chat.get("id")?.as_i64()?.to_string();
    let username = from
        .and_then(|from| from.get("username"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let data = callback.get("data")?.as_str()?.to_string();
    Some(IncomingCallback {
        update_id,
        callback_id,
        chat_id,
        username,
        message_id,
        data,
    })
}

pub fn allowed(allowlist: &[String], chat_id: &str, username: Option<&str>) -> bool {
    let chat_id = chat_id.trim();
    let username = username.map(normalize_username);
    allowlist.iter().any(|entry| {
        let entry = entry.trim();
        if entry.is_empty() {
            return false;
        }
        if is_chat_id(entry) {
            entry == chat_id
        } else {
            username
                .as_deref()
                .map(|username| normalize_username(entry) == username)
                .unwrap_or(false)
        }
    })
}

pub(crate) fn is_chat_id(value: &str) -> bool {
    let value = value.strip_prefix('-').unwrap_or(value);
    !value.is_empty() && value.chars().all(|character| character.is_ascii_digit())
}

pub(crate) fn normalize_username(value: &str) -> String {
    value.trim().trim_start_matches('@').to_lowercase()
}

pub fn chunks(text: &str, limit: usize) -> Vec<String> {
    let limit = limit.max(200);
    if text.chars().count() <= limit {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if current.chars().count() + line.chars().count() + 1 > limit && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
        }
        if line.chars().count() > limit {
            for piece in split_hard(line, limit) {
                if !current.is_empty() {
                    chunks.push(std::mem::take(&mut current));
                }
                chunks.push(piece);
            }
            continue;
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    if chunks.is_empty() {
        chunks.push(String::new());
    }
    chunks
}

fn split_hard(value: &str, limit: usize) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    for character in value.chars() {
        if current.chars().count() >= limit {
            parts.push(std::mem::take(&mut current));
        }
        current.push(character);
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

/// Parses `#abcdef12 2` into (short code, answer). The code is at least four
/// characters so it cannot collide with ordinary hashtags.
pub fn parse_interaction_reply(text: &str) -> Option<(&str, &str)> {
    let rest = text.trim().strip_prefix('#')?;
    let (short, answer) = rest.split_once(char::is_whitespace)?;
    let short = short.trim();
    if short.len() < 4 || !short.chars().all(|c| c.is_ascii_hexdigit() || c == '-') {
        return None;
    }
    Some((short, answer.trim()))
}

pub fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

/// `input/` is scanned into every task prompt, so names must stay portable and
/// cannot escape the workspace.
pub fn safe_file_name(value: &str) -> String {
    let base = value.rsplit(['/', '\\']).next().unwrap_or(value);
    let mut name = base
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_' | ' ') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .trim_start_matches('.')
        .to_string();
    if name.is_empty() {
        name = "arquivo".into();
    }
    name.chars().take(100).collect()
}

pub fn truncate(value: &str, limit: usize) -> String {
    let mut output = value.chars().take(limit).collect::<String>();
    if value.chars().count() > limit {
        output.push('…');
    }
    output
}

pub fn help_text(agent: &str) -> String {
    format!(
        "OpenDesk conectado.\n\n\
         Converse com @{agent} em linguagem natural. Exemplos:\n\
         - crie uma tarefa para revisar o relatório de outubro;\n\
         - mostre o status das execuções e da fila;\n\
         - execute a rotina <id> ou a receita <id>;\n\
         - qual foi o resultado da tarefa <id>?\n\n\
         Comandos:\n\
         /help — esta ajuda\n\
         aprovar <prefixo> — aprova uma permissão pendente\n\
         negar <prefixo> — nega uma permissão pendente\n\
         #<código> <opção> — responde uma pergunta HITL de tarefa\n\
         Para aprovações de receita, responda com o token OD-… e APROVAR/REJEITAR."
    )
}
