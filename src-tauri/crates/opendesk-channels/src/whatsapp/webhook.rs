use serde_json::Value;
use sha2::{Digest, Sha256};

const HMAC_BLOCK: usize = 64;

#[derive(Debug, Clone)]
pub struct MetaMessage {
    pub identity: String,
    pub text: String,
    pub message_id: Option<String>,
    pub name: Option<String>,
    pub media_id: Option<String>,
    pub media_mime: Option<String>,
    pub media_name: Option<String>,
}

/// Extracts text, button, interactive and media messages from a Meta
/// `whatsapp_business_account` payload. Entries without a usable text are
/// skipped, matching the bridge's behaviour.
pub fn extract_meta_messages(value: &Value) -> Vec<MetaMessage> {
    let mut messages = Vec::new();
    let Some(entries) = value.get("entry").and_then(Value::as_array) else {
        return messages;
    };
    for entry in entries {
        let Some(changes) = entry.get("changes").and_then(Value::as_array) else {
            continue;
        };
        for change in changes {
            let Some(value) = change.get("value") else {
                continue;
            };
            let contacts = value
                .get("contacts")
                .and_then(Value::as_array)
                .map(|contacts| {
                    contacts
                        .iter()
                        .filter_map(|contact| {
                            let id = contact.get("wa_id").and_then(Value::as_str)?;
                            let name = contact
                                .pointer("/profile/name")
                                .and_then(Value::as_str)
                                .map(str::to_string);
                            Some((id.to_string(), name))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let Some(items) = value.get("messages").and_then(Value::as_array) else {
                continue;
            };
            for message in items {
                let Some(identity) = message.get("from").and_then(Value::as_str) else {
                    continue;
                };
                let kind = message.get("type").and_then(Value::as_str).unwrap_or("");
                let media = message.get(kind).and_then(Value::as_object);
                let text = match kind {
                    "text" => message
                        .pointer("/text/body")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    "button" => message
                        .pointer("/button/text")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    "interactive" => message
                        .pointer("/interactive/button_reply/title")
                        .or_else(|| message.pointer("/interactive/list_reply/title"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string(),
                    "image" | "document" | "video" | "audio" | "voice" | "sticker" => message
                        .pointer("/image/caption")
                        .or_else(|| message.pointer("/document/caption"))
                        .or_else(|| message.pointer("/video/caption"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .unwrap_or_else(|| format!("[{kind} recebido]")),
                    "" => continue,
                    other => format!("[{other} recebido]"),
                };
                if text.trim().is_empty() {
                    continue;
                }
                let name = contacts
                    .iter()
                    .find(|(id, _)| id == identity)
                    .and_then(|(_, name)| name.clone());
                messages.push(MetaMessage {
                    identity: identity.to_string(),
                    text,
                    message_id: message
                        .get("id")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    name,
                    media_id: media
                        .and_then(|media| media.get("id"))
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    media_mime: media
                        .and_then(|media| media.get("mime_type"))
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    media_name: media
                        .and_then(|media| media.get("filename"))
                        .and_then(Value::as_str)
                        .map(str::to_string),
                });
            }
        }
    }
    messages
}

/// Verifies Meta's `X-Hub-Signature-256` header (`sha256=<hex>`).
pub fn verify_signature(secret: &str, body: &[u8], header: &str) -> bool {
    let Some(encoded) = header.strip_prefix("sha256=") else {
        return false;
    };
    let Some(expected) = hex_decode(encoded) else {
        return false;
    };
    hmac_sha256(secret.as_bytes(), body).as_slice() == expected.as_slice()
}

/// RFC 2104 HMAC-SHA256 (the algorithm Meta uses for `X-Hub-Signature-256`).
fn hmac_sha256(key: &[u8], body: &[u8]) -> [u8; 32] {
    let mut block = [0u8; HMAC_BLOCK];
    if key.len() > HMAC_BLOCK {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }

    let mut inner = Sha256::new();
    let mut outer = Sha256::new();
    let mut inner_pad = [0x36u8; HMAC_BLOCK];
    let mut outer_pad = [0x5cu8; HMAC_BLOCK];
    for index in 0..HMAC_BLOCK {
        inner_pad[index] ^= block[index];
        outer_pad[index] ^= block[index];
    }
    inner.update(inner_pad);
    inner.update(body);
    outer.update(outer_pad);
    outer.update(inner.finalize());

    let mut digest = [0u8; 32];
    digest.copy_from_slice(&outer.finalize());
    digest
}

pub(crate) fn hex_decode(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}

/// Derives a safe file extension for downloaded media, preferring the original
/// filename and falling back to the MIME type.
pub fn media_extension(mime: Option<&str>, filename: Option<&str>) -> String {
    if let Some(filename) = filename
        && let Some(extension) = std::path::Path::new(filename)
            .extension()
            .and_then(|value| value.to_str())
    {
        let extension = extension
            .chars()
            .filter(|character| character.is_ascii_alphanumeric())
            .take(8)
            .collect::<String>();
        if !extension.is_empty() {
            return extension.to_lowercase();
        }
    }
    match mime.unwrap_or("").split(';').next().unwrap_or("") {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "audio/ogg" => "ogg",
        "audio/mpeg" => "mp3",
        "audio/mp4" => "m4a",
        "audio/wav" => "wav",
        "video/mp4" => "mp4",
        "application/pdf" => "pdf",
        _ => "bin",
    }
    .to_string()
}
