//! Telegram configuration, token/offset storage and known chats.

use crate::error::{AppError, AppResult};
use opendesk_storage::{repos::settings, secrets::SecretStore};
use sqlx::SqlitePool;
use std::collections::HashSet;

use super::parse::{is_chat_id, normalize_username};
use super::types::{KnownChat, TelegramConfig};

const SETTING_KEY: &str = "telegram_config";
const TOKEN_NAMESPACE: &str = "telegram";
const TOKEN_KEY: &str = "bot_token";
pub const OFFSET_KEY: &str = "telegram_last_update_id";
const CHATS_KEY: &str = "telegram_chats";

pub async fn load(pool: &SqlitePool) -> AppResult<TelegramConfig> {
    let value = settings::get(pool, SETTING_KEY).await?;
    Ok(value
        .and_then(|value| serde_json::from_value::<TelegramConfig>(value).ok())
        .unwrap_or_default())
}

pub async fn save(pool: &SqlitePool, config: &TelegramConfig) -> AppResult<()> {
    settings::set(pool, SETTING_KEY, &serde_json::to_value(config)?)
        .await
        .map_err(Into::into)
}

pub fn token(secrets: &SecretStore) -> Option<String> {
    secrets
        .get(TOKEN_NAMESPACE, TOKEN_KEY)
        .ok()
        .flatten()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn token_for_parts(secrets: &SecretStore) -> AppResult<String> {
    token(secrets).ok_or_else(|| {
        AppError::BadRequest("Telegram não está configurado: informe o token do bot.".into())
    })
}

/// Accepts the canonical `<bot_id>:<secret>` shape and rejects obvious paste
/// mistakes without being stricter than Telegram itself.
pub fn valid_token_shape(value: &str) -> bool {
    let value = value.trim();
    let Some((id, secret)) = value.split_once(':') else {
        return false;
    };
    id.chars().all(|character| character.is_ascii_digit())
        && !id.is_empty()
        && secret.len() >= 20
        && secret
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
}

pub async fn clear_offset(pool: &SqlitePool) {
    let _ = sqlx::query("DELETE FROM app_settings WHERE key = ?")
        .bind(OFFSET_KEY)
        .execute(pool)
        .await;
}

pub async fn record_chat(
    pool: &SqlitePool,
    chat_id: &str,
    username: Option<&str>,
    title: Option<&str>,
) -> AppResult<()> {
    let mut chats = settings::get(pool, CHATS_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_value::<Vec<KnownChat>>(value).ok())
        .unwrap_or_default();
    if let Some(chat) = chats.iter_mut().find(|chat| chat.chat_id == chat_id) {
        chat.username = username.map(str::to_string).or(chat.username.clone());
        chat.title = title.map(str::to_string).or(chat.title.clone());
    } else {
        chats.push(KnownChat {
            chat_id: chat_id.to_string(),
            username: username.map(str::to_string),
            title: title.map(str::to_string),
        });
    }
    settings::set(pool, CHATS_KEY, &serde_json::to_value(chats)?)
        .await
        .map_err(Into::into)
}

pub async fn known_chats(pool: &SqlitePool) -> Vec<KnownChat> {
    settings::get(pool, CHATS_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_value::<Vec<KnownChat>>(value).ok())
        .unwrap_or_default()
}

/// Resolve numeric chat IDs and `@username` entries against chats the bot has
/// already seen. Telegram cannot send to a private username before the user
/// messages the bot.
pub async fn resolve_recipients(pool: &SqlitePool, recipients: &[String]) -> Vec<String> {
    let chats = known_chats(pool).await;
    let mut seen = HashSet::new();
    let mut resolved = Vec::new();
    for entry in recipients {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let chat_id = if is_chat_id(entry) {
            Some(entry.to_string())
        } else {
            let username = normalize_username(entry);
            chats
                .iter()
                .find(|chat| {
                    chat.username
                        .as_deref()
                        .map(|value| normalize_username(value) == username)
                        .unwrap_or(false)
                })
                .map(|chat| chat.chat_id.clone())
        };
        if let Some(chat_id) = chat_id
            && seen.insert(chat_id.clone())
        {
            resolved.push(chat_id);
        }
    }
    resolved
}
