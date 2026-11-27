//! Background chat reply and inbound file handling for Telegram.

use super::http::TypingGuard;
use super::{
    IncomingTelegram, IncomingTelegramFile, MAX_FILE_BYTES, download_file, safe_file_name,
    send_reply, send_text,
};
use crate::{
    app_state::AppState,
    chat,
    db::workspaces,
    error::{AppError, AppResult},
};
use std::{path::Path, sync::Arc, time::Duration};

pub(super) struct ReplyRequest {
    pub token: String,
    pub chat_id: String,
    pub message_id: i64,
    pub thread: crate::db::chat::ChatThread,
    pub content: String,
    pub context: String,
    pub context_id: Option<String>,
}

pub(super) async fn run_chat_reply(state: Arc<AppState>, request: ReplyRequest) {
    let ReplyRequest {
        token,
        chat_id,
        message_id,
        thread,
        content,
        context,
        context_id,
    } = request;
    let typing = TypingGuard::start(token.clone(), chat_id.clone());
    let progress = tokio::spawn({
        let token = token.clone();
        let chat_id = chat_id.clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_secs(120)).await;
                let _ = send_text(&token, &chat_id, "Ainda estou trabalhando…").await;
            }
        }
    });
    let thread_id = thread.id.clone();
    let result = chat::send_message_with_context(&state, thread, &content, Some(&context)).await;
    progress.abort();
    if let Some(context_id) = context_id
        && let Ok(updated) = crate::db::chat::get_thread(&state.db, &thread_id).await
    {
        let _ = crate::db::channel_bots::update_context(
            &state.db,
            &context_id,
            crate::db::channel_bots::ChannelChatContextInput {
                opencode_session_id: Some(updated.opencode_session_id),
                ..Default::default()
            },
        )
        .await;
    }
    drop(typing);
    match result {
        Ok(reply) => {
            let _ = send_reply(&token, &chat_id, message_id, &reply.assistant.content).await;
        }
        Err(error) => {
            let _ = send_reply(
                &token,
                &chat_id,
                message_id,
                &format!("Não consegui responder agora: {error}"),
            )
            .await;
        }
    }
}

pub(super) async fn save_incoming_file(
    state: &Arc<AppState>,
    workspace_id: &str,
    incoming: &IncomingTelegram,
    file: &IncomingTelegramFile,
    token: &str,
) -> AppResult<String> {
    if file.size.is_some_and(|size| size > MAX_FILE_BYTES as i64) {
        return Err(AppError::BadRequest(format!(
            "o arquivo excede o limite de {} MB do Bot API",
            MAX_FILE_BYTES / (1024 * 1024)
        )));
    }
    let workspace = workspaces::get(&state.db, workspace_id).await?;
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let relative_dir = Path::new("input").join("telegram").join(&day);
    let file_name = safe_file_name(&format!(
        "{}-{}-{}",
        incoming.chat_id, incoming.message_id, file.file_name
    ));
    let relative = relative_dir.join(file_name);
    let destination = Path::new(&workspace.root_path).join(&relative);
    download_file(token, &file.file_id, &destination).await?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}
