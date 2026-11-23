//! WhatsApp Web personal bridge host wiring.

use crate::{app_state::AppState, db::settings, error::AppResult};
use opendesk_channels::whatsapp::{
    QrEvent, WhatsAppQrBridge as InnerBridge, WhatsAppQrConfig, WhatsAppQrEventHandler,
    load_qr_config, qr_allowed,
};
use serde_json::{Value, json};
use std::{future::Future, pin::Pin, sync::Arc};

const LAST_QR: &str = "whatsapp_qr_last_qr";
const LAST_STATUS: &str = "whatsapp_qr_last_status";

#[derive(Default)]
pub struct WhatsAppQrBridge {
    inner: Arc<InnerBridge>,
}

impl WhatsAppQrBridge {
    pub async fn is_running(&self) -> bool {
        self.inner.is_running().await
    }

    pub async fn stop(&self) {
        self.inner.stop().await;
    }

    pub async fn start_if_enabled(self: &Arc<Self>, state: Arc<AppState>) -> AppResult<()> {
        let config = load_qr_config(&state.db).await?;
        if !config.enabled {
            self.inner.stop().await;
            return Ok(());
        }
        let handler: Arc<dyn WhatsAppQrEventHandler> = Arc::new(AppQrHandler {
            state: Arc::clone(&state),
        });
        self.inner.start(config, handler).await?;
        Ok(())
    }

    pub async fn send_text(&self, to: &str, text: &str) -> AppResult<()> {
        self.inner.send_text(to, text).await.map_err(Into::into)
    }
}

pub async fn status(state: &AppState) -> AppResult<Value> {
    let config = load_qr_config(&state.db).await?;
    let qr = settings::get(&state.db, LAST_QR).await?;
    let status = settings::get(&state.db, LAST_STATUS).await?;
    Ok(json!({
        "config": config,
        "running": state.whatsapp_qr.is_running().await,
        "last_qr": qr.and_then(|value| value.as_str().map(str::to_string)),
        "last_status": status.and_then(|value| value.as_str().map(str::to_string)),
    }))
}

struct AppQrHandler {
    state: Arc<AppState>,
}

impl WhatsAppQrEventHandler for AppQrHandler {
    fn handle<'a>(
        &'a self,
        config: WhatsAppQrConfig,
        event: QrEvent,
    ) -> Pin<Box<dyn Future<Output = opendesk_channels::error::AppResult<()>> + Send + 'a>> {
        Box::pin(async move {
            match event {
                QrEvent::Qr(qr) => {
                    settings::set(&self.state.db, LAST_QR, &Value::String(qr)).await?;
                }
                QrEvent::Status(status) => {
                    settings::set(&self.state.db, LAST_STATUS, &Value::String(status)).await?;
                }
                QrEvent::Error(error) => {
                    settings::set(
                        &self.state.db,
                        LAST_STATUS,
                        &Value::String(format!("erro: {error}")),
                    )
                    .await?;
                }
                QrEvent::Message { from, text, media } => {
                    if !qr_allowed(&config, &from) {
                        return Ok(());
                    }
                    let mut text = text;
                    if !media.is_empty() {
                        text.push_str("\n\nMídia:\n");
                        for url in &media {
                            text.push_str(&format!("- {url}\n"));
                        }
                    }
                    let response = crate::integrations::external_chat::reply(
                        &self.state,
                        "whatsapp-qr",
                        &from,
                        &text,
                        &config.agent,
                        config.workspace_id.as_deref(),
                    )
                    .await?;
                    let _ = self.state.whatsapp_qr.send_text(&from, &response).await;
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
    async fn disabled_bridge_stays_stopped_and_send_fails() {
        let ctx = crate::test_support::context().await;
        let bridge = std::sync::Arc::new(WhatsAppQrBridge::default());
        bridge
            .start_if_enabled(ctx.state.clone())
            .await
            .expect("start");
        assert!(!bridge.is_running().await);
        assert!(bridge.send_text("5511", "oi").await.is_err());
    }

    #[tokio::test]
    async fn enabled_qr_without_sidecar_is_rejected() {
        let ctx = crate::test_support::context().await;
        opendesk_channels::whatsapp::save_qr_config(
            &ctx.state.db,
            &opendesk_channels::whatsapp::WhatsAppQrConfig {
                enabled: true,
                ..Default::default()
            },
        )
        .await
        .expect("save");
        let bridge = std::sync::Arc::new(WhatsAppQrBridge::default());
        assert!(bridge.start_if_enabled(ctx.state.clone()).await.is_err());
    }
}
