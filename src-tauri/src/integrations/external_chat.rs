//! Shared inbound chat routing for Discord, Slack and WhatsApp QR.

use crate::{
    app_state::AppState,
    chat,
    db::{chat as chat_db, settings},
    error::{AppError, AppResult},
};
use std::sync::Arc;

pub async fn reply(
    state: &Arc<AppState>,
    channel: &str,
    external_id: &str,
    text: &str,
    agent: &str,
    workspace_id: Option<&str>,
) -> AppResult<String> {
    let workspace_id = match workspace_id {
        Some(id) => id.to_string(),
        None => settings::active_workspace_id(&state.db)
            .await?
            .ok_or_else(|| AppError::BadRequest("sem workspace ativo".into()))?,
    };
    let thread =
        match chat_db::find_external(&state.db, &workspace_id, channel, external_id).await? {
            Some(thread) => thread,
            None => {
                chat_db::create_thread(
                    &state.db,
                    chat_db::CreateThreadInput {
                        workspace_id: workspace_id.clone(),
                        title: Some(format!("{channel} · {external_id}")),
                        agent: agent.to_string(),
                        model: None,
                        source: Some(channel.to_string()),
                        external_id: Some(external_id.to_string()),
                    },
                )
                .await?
            }
        };
    let reply = chat::send_message_with_context(state, thread, text, None).await?;
    Ok(if reply.assistant.content.trim().is_empty() {
        "Recebi sua mensagem.".to_string()
    } else {
        reply.assistant.content
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reports_missing_workspace_or_daemon() {
        let ctx = crate::test_support::context().await;
        let result = reply(
            &ctx.state,
            "discord",
            "channel-1",
            "oi",
            "build",
            Some(&ctx.workspace_id),
        )
        .await;
        assert!(result.is_err());
    }
}
