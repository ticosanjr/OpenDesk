//! Telegram DTOs and configuration types.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default = "default_agent")]
    pub agent: String,
}

fn default_agent() -> String {
    "build".into()
}

impl Default for TelegramConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            allowlist: Vec::new(),
            workspace_id: None,
            agent: default_agent(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TelegramStatus {
    pub config: TelegramConfig,
    pub token_configured: bool,
    pub bot_username: Option<String>,
    pub bot_name: Option<String>,
    pub bridge_running: bool,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotInfo {
    pub id: i64,
    pub is_bot: bool,
    #[serde(default)]
    pub first_name: String,
    #[serde(default)]
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnownChat {
    pub chat_id: String,
    pub username: Option<String>,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingTelegramFile {
    pub file_id: String,
    pub file_name: String,
    pub mime_type: Option<String>,
    pub size: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingTelegram {
    pub update_id: i64,
    pub message_id: i64,
    pub chat_id: String,
    pub username: Option<String>,
    pub chat_title: Option<String>,
    pub text: String,
    pub file: Option<IncomingTelegramFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingCallback {
    pub update_id: i64,
    pub callback_id: String,
    pub chat_id: String,
    pub username: Option<String>,
    pub message_id: i64,
    pub data: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramUpdate {
    Message(IncomingTelegram),
    Callback(IncomingCallback),
}
