//! Chat-scoped personal context and structured memory resolution.

use crate::{app_state::AppState, db::chat::ChatThread, error::AppResult};
use opendesk_domain::MemoryUsageScope;

#[derive(Debug, Default)]
pub struct ChatMemoryBodies {
    pub personal: String,
    pub structured: String,
}

/// Resolves the memory scope for a chat thread. Bot groups may be configured
/// with `none`; every other chat reads `chat` and `both` records only.
pub async fn chat_memory_bodies(
    state: &AppState,
    thread: &ChatThread,
) -> AppResult<ChatMemoryBodies> {
    let scope = thread_memory_scope(state, thread).await;
    if scope == "none" {
        return Ok(ChatMemoryBodies::default());
    }
    let personal = crate::personal_context::render_body_chat(&state.db, &thread.workspace_id)
        .await
        .unwrap_or_default();
    let structured = crate::db::memory::render_context_for(
        &state.db,
        &thread.workspace_id,
        &thread.agent,
        "",
        5,
        Some(MemoryUsageScope::Chat),
    )
    .await
    .unwrap_or_default();
    Ok(ChatMemoryBodies {
        personal,
        structured,
    })
}

async fn thread_memory_scope(state: &AppState, thread: &ChatThread) -> String {
    if !matches!(thread.source.as_str(), "telegram" | "signal") {
        return "chat".into();
    }
    let Some(external_id) = thread.external_id.as_deref() else {
        return "chat".into();
    };
    let Some((_channel, rest)) = external_id.split_once(':') else {
        return "chat".into();
    };
    let Some((bot_id, chat_id)) = rest.split_once(':') else {
        return "chat".into();
    };
    crate::db::channel_bots::find_context(&state.db, bot_id, chat_id)
        .await
        .ok()
        .flatten()
        .map(|context| context.memory_scope)
        .unwrap_or_else(|| "chat".into())
}
