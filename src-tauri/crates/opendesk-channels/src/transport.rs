//! Common transport contract for channels that can send text and HITL.

use crate::error::AppResult;
use async_trait::async_trait;
use std::sync::Arc;

#[async_trait]
pub trait ChannelTransport: Send + Sync {
    fn id(&self) -> &'static str;
    async fn send_text(&self, target: &str, text: &str) -> AppResult<String>;
    async fn send_hitl(
        &self,
        target: &str,
        title: &str,
        body: &str,
        actions: &[(String, String)],
    ) -> AppResult<String>;
}

#[cfg(feature = "ch-discord")]
pub struct DiscordTransport {
    pub token: String,
}

#[cfg(feature = "ch-discord")]
#[async_trait]
impl ChannelTransport for DiscordTransport {
    fn id(&self) -> &'static str {
        "discord"
    }

    async fn send_text(&self, target: &str, text: &str) -> AppResult<String> {
        crate::discord::send_text(&self.token, target, text).await?;
        Ok("discord".into())
    }

    async fn send_hitl(
        &self,
        target: &str,
        title: &str,
        body: &str,
        actions: &[(String, String)],
    ) -> AppResult<String> {
        crate::discord::send_hitl(&self.token, target, title, body, actions).await?;
        Ok("discord".into())
    }
}

#[cfg(feature = "ch-slack")]
pub struct SlackTransport {
    pub bot_token: String,
}

#[cfg(feature = "ch-slack")]
#[async_trait]
impl ChannelTransport for SlackTransport {
    fn id(&self) -> &'static str {
        "slack"
    }

    async fn send_text(&self, target: &str, text: &str) -> AppResult<String> {
        crate::slack::send_text(&self.bot_token, target, text).await?;
        Ok("slack".into())
    }

    async fn send_hitl(
        &self,
        target: &str,
        title: &str,
        body: &str,
        actions: &[(String, String)],
    ) -> AppResult<String> {
        crate::slack::send_hitl(&self.bot_token, target, title, body, actions).await?;
        Ok("slack".into())
    }
}

#[cfg(all(feature = "ch-whatsapp", feature = "ch-whatsapp-qr"))]
pub struct WhatsAppQrTransport {
    pub bridge: Arc<crate::whatsapp::WhatsAppQrBridge>,
}

#[cfg(all(feature = "ch-whatsapp", feature = "ch-whatsapp-qr"))]
#[async_trait]
impl ChannelTransport for WhatsAppQrTransport {
    fn id(&self) -> &'static str {
        "whatsapp-qr"
    }

    async fn send_text(&self, target: &str, text: &str) -> AppResult<String> {
        self.bridge.send_text(target, text).await?;
        Ok("whatsapp-qr".into())
    }

    async fn send_hitl(
        &self,
        target: &str,
        title: &str,
        body: &str,
        actions: &[(String, String)],
    ) -> AppResult<String> {
        let labels = actions
            .iter()
            .map(|(_, label)| label.as_str())
            .collect::<Vec<_>>()
            .join(" / ");
        self.bridge
            .send_text(target, &format!("{title}\n{body}\n{labels}"))
            .await?;
        Ok("whatsapp-qr".into())
    }
}
