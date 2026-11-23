use crate::{app_state::AppState, db::io_bus::InboundSource, error::AppResult};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub mod chat_bridge;
pub mod dropzone;
pub mod imap_idle;
pub mod sql_poller;
pub mod webhook;

pub async fn run(
    state: Arc<AppState>,
    source: InboundSource,
    cancel: CancellationToken,
) -> AppResult<()> {
    let config: serde_json::Value = crate::io_bus::source_config(&source)?;
    crate::io_bus::secrets_guard::validate_no_plaintext_secrets(&config)?;
    match source.source_type.as_str() {
        "FOLDER_WATCH" | "OCR_WATCH" | "AUDIO_STT_WATCH" => {
            dropzone::run(state, source, cancel).await
        }
        "SQL_POLLER" => sql_poller::run(state, source, cancel).await,
        "IMAP_IDLE" => imap_idle::run(state, source, cancel).await,
        "TELEGRAM_BOT" | "MATRIX_BOT" | "MATTERMOST_BOT" => {
            chat_bridge::run(state, source, cancel).await
        }
        other => {
            tracing::warn!(source_type = other, "webhook source needs no worker");
            Ok(())
        }
    }
}
