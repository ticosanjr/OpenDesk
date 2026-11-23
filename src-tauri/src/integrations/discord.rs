//! Discord host wiring: bridge lifecycle, config and inbound routing.

use crate::{app_state::AppState, error::AppResult};
use opendesk_channels::discord::{
    self as channel, DiscordBridge as InnerBridge, DiscordConfig, DiscordEvent, DiscordEventHandler,
};
use std::{future::Future, pin::Pin, sync::Arc};

#[derive(Default)]
pub struct DiscordBridge {
    inner: Arc<InnerBridge>,
}

impl DiscordBridge {
    pub async fn is_running(&self) -> bool {
        self.inner.is_running().await
    }

    pub async fn stop(&self) {
        self.inner.stop().await;
    }

    pub async fn start_if_enabled(self: &Arc<Self>, state: Arc<AppState>) -> AppResult<()> {
        let config = channel::load(&state.db).await?;
        if !config.enabled {
            self.inner.stop().await;
            return Ok(());
        }
        let token = channel::token(&state.secrets)
            .ok_or_else(|| crate::error::AppError::BadRequest("token do Discord ausente".into()))?;
        let handler: Arc<dyn DiscordEventHandler> = Arc::new(AppDiscordHandler {
            state: Arc::clone(&state),
        });
        self.inner.start(config, token, handler).await;
        Ok(())
    }
}

struct AppDiscordHandler {
    state: Arc<AppState>,
}

impl DiscordEventHandler for AppDiscordHandler {
    fn handle<'a>(
        &'a self,
        config: DiscordConfig,
        event: DiscordEvent,
    ) -> Pin<Box<dyn Future<Output = opendesk_channels::error::AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            let token = channel::token(&self.state.secrets);
            match event {
                DiscordEvent::Message(message) => {
                    let mut content = message.content.clone();
                    if !message.attachments.is_empty() {
                        content.push_str("\n\nAnexos:\n");
                        for url in &message.attachments {
                            content.push_str(&format!("- {url}\n"));
                        }
                    }
                    let response = crate::integrations::external_chat::reply(
                        &self.state,
                        "discord",
                        &message.channel_id,
                        &content,
                        &config.agent,
                        config.workspace_id.as_deref(),
                    )
                    .await?;
                    if let Some(token) = token
                        && let Err(error) = channel::send_text_reply(
                            &token,
                            &message.channel_id,
                            &response,
                            &message.message_id,
                        )
                        .await
                    {
                        tracing::warn!(error = %error, "Discord reply failed");
                    }
                }
                DiscordEvent::Interaction(action) => {
                    let mut parts = action.custom_id.splitn(3, ':');
                    if parts.next() == Some("hitl")
                        && let Some(token) = parts.next()
                    {
                        let decision = parts.next().unwrap_or("answer");
                        let context = crate::app_state::recipe_context(&self.state);
                        let resolved = crate::recipes::hitl::resolve(
                            &context,
                            token,
                            "DISCORD",
                            Some(&action.user_id),
                            decision,
                            None,
                        )
                        .await
                        .unwrap_or(None);
                        if resolved.is_none() {
                            let _ = crate::api::interactions::resolve_internal(
                                &self.state,
                                token,
                                Vec::new(),
                                action.response.clone(),
                                Some(decision.to_string()),
                            )
                            .await;
                        }
                    }
                    if let Some(token) = token {
                        let _ =
                            channel::send_text(&token, &action.channel_id, "Decisão registrada.")
                                .await;
                    }
                }
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disabled_bridge_stays_stopped() {
        let ctx = crate::test_support::context().await;
        let bridge = std::sync::Arc::new(DiscordBridge::default());
        bridge
            .start_if_enabled(ctx.state.clone())
            .await
            .expect("start");
        assert!(!bridge.is_running().await);
    }

    #[tokio::test]
    async fn enabled_bridge_without_token_is_rejected() {
        let ctx = crate::test_support::context().await;
        opendesk_channels::discord::save(
            &ctx.state.db,
            &opendesk_channels::discord::DiscordConfig {
                enabled: true,
                ..Default::default()
            },
        )
        .await
        .expect("save");
        let bridge = std::sync::Arc::new(DiscordBridge::default());
        assert!(bridge.start_if_enabled(ctx.state.clone()).await.is_err());
    }
}
