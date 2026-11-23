//! Slack host wiring: Socket Mode bridge, config and inbound routing.

use crate::{app_state::AppState, error::AppResult};
use opendesk_channels::slack::{
    self as channel, SlackAction, SlackBridge as InnerBridge, SlackConfig, SlackEvent,
    SlackEventHandler,
};
use std::{future::Future, pin::Pin, sync::Arc};

#[derive(Default)]
pub struct SlackBridge {
    inner: Arc<InnerBridge>,
}

impl SlackBridge {
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
        let app_token = channel::app_token(&state.secrets).ok_or_else(|| {
            crate::error::AppError::BadRequest("app token do Slack ausente".into())
        })?;
        let handler: Arc<dyn SlackEventHandler> = Arc::new(AppSlackHandler {
            state: Arc::clone(&state),
        });
        self.inner.start(config, app_token, handler).await;
        Ok(())
    }
}

struct AppSlackHandler {
    state: Arc<AppState>,
}

impl SlackEventHandler for AppSlackHandler {
    fn handle<'a>(
        &'a self,
        config: SlackConfig,
        event: SlackEvent,
    ) -> Pin<Box<dyn Future<Output = opendesk_channels::error::AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            let token = channel::bot_token(&self.state.secrets);
            match event {
                SlackEvent::Message(message) => {
                    let mut text = message.text.clone();
                    if !message.files.is_empty() {
                        text.push_str("\n\nArquivos:\n");
                        for url in &message.files {
                            text.push_str(&format!("- {url}\n"));
                        }
                    }
                    let response = crate::integrations::external_chat::reply(
                        &self.state,
                        "slack",
                        &message.channel_id,
                        &text,
                        &config.agent,
                        config.workspace_id.as_deref(),
                    )
                    .await?;
                    if let Some(token) = token {
                        let result = if let Some(thread) = message.thread_ts.as_deref() {
                            channel::send_text_in_thread(
                                &token,
                                &message.channel_id,
                                &response,
                                thread,
                            )
                            .await
                        } else {
                            channel::send_text(&token, &message.channel_id, &response).await
                        };
                        if let Err(error) = result {
                            tracing::warn!(error = %error, "Slack reply failed");
                        }
                    }
                }
                SlackEvent::Action(action) => {
                    resolve_action(&self.state, &action).await;
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

async fn resolve_action(state: &Arc<AppState>, action: &SlackAction) {
    let mut parts = action.action_id.splitn(3, ':');
    if parts.next() == Some("hitl")
        && let Some(token) = parts.next()
    {
        let decision = parts.next().unwrap_or("answer");
        let context = crate::app_state::recipe_context(state);
        let resolved = crate::recipes::hitl::resolve(
            &context,
            token,
            "SLACK",
            Some(&action.user_id),
            decision,
            None,
        )
        .await
        .unwrap_or(None);
        if resolved.is_none() {
            let _ = crate::api::interactions::resolve_internal(
                state,
                token,
                Vec::new(),
                action.value.clone(),
                Some(decision.to_string()),
            )
            .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disabled_bridge_stays_stopped() {
        let ctx = crate::test_support::context().await;
        let bridge = std::sync::Arc::new(SlackBridge::default());
        bridge
            .start_if_enabled(ctx.state.clone())
            .await
            .expect("start");
        assert!(!bridge.is_running().await);
    }

    #[tokio::test]
    async fn enabled_bridge_without_token_is_rejected() {
        let ctx = crate::test_support::context().await;
        opendesk_channels::slack::save(
            &ctx.state.db,
            &opendesk_channels::slack::SlackConfig {
                enabled: true,
                ..Default::default()
            },
        )
        .await
        .expect("save");
        let bridge = std::sync::Arc::new(SlackBridge::default());
        assert!(bridge.start_if_enabled(ctx.state.clone()).await.is_err());
    }
}
