mod format;
mod host;
mod http;
mod reply;
#[cfg(test)]
mod tests;

use crate::{app_state::AppState, error::AppResult};

pub use format::{notify_permission, notify_task_interaction};
pub use host::{TelegramBridge, handle_bot_update};

#[allow(unused_imports)]
pub use opendesk_channels::telegram::{
    BotInfo, IncomingCallback, IncomingTelegram, IncomingTelegramFile, KnownChat, MAX_FILE_BYTES,
    OFFSET_KEY, TelegramConfig, TelegramStatus, TelegramUpdate, allowed, answer_callback_query,
    chunks, clear_buttons, clear_offset, delete_webhook, download_file, get_me, get_updates,
    help_text, known_chats, load, parse_callback_update, parse_interaction_reply, parse_update,
    parse_updates, record_chat, resolve_recipients, safe_file_name, save, send_chat_action,
    send_document, send_reply, send_text, send_text_with_markup, send_to_allowlist_parts, short_id,
    token, token_for_parts, truncate, valid_token_shape,
};

pub async fn token_for(state: &AppState) -> AppResult<String> {
    token_for_parts(&state.secrets).map_err(Into::into)
}

pub async fn status(state: &AppState) -> TelegramStatus {
    let config = load(&state.db).await.unwrap_or_default();
    let token = token(&state.secrets);
    let bot = state.telegram.bot().await;
    TelegramStatus {
        config,
        token_configured: token.is_some(),
        bot_username: bot.as_ref().and_then(|bot| bot.username.clone()),
        bot_name: bot.map(|bot| bot.first_name),
        bridge_running: state.telegram.is_running().await,
        last_error: state.telegram.last_error().await,
    }
}
