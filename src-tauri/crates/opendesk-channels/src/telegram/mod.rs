//! Telegram Bot API transport, configuration and pure parsing.
//!
//! The shell keeps only the host orchestration that needs `AppState`; this
//! module owns everything that can run without it.

mod client;
mod config;
mod parse;
mod types;

#[cfg(test)]
mod tests;

pub use client::{
    MAX_FILE_BYTES, answer_callback_query, clear_buttons, delete_webhook, download_file, get_me,
    get_updates, get_updates_value, send_chat_action, send_document, send_reply, send_text,
    send_text_with_markup, send_text_with_parse_mode, send_to_allowlist_parts,
};
pub use config::{
    OFFSET_KEY, clear_offset, known_chats, load, record_chat, resolve_recipients, save, token,
    token_for_parts, valid_token_shape,
};
pub use parse::{
    allowed, chunks, help_text, parse_callback_update, parse_interaction_reply, parse_update,
    parse_updates, safe_file_name, short_id, truncate,
};
pub use types::{
    BotInfo, IncomingCallback, IncomingTelegram, IncomingTelegramFile, KnownChat, TelegramConfig,
    TelegramStatus, TelegramUpdate,
};
