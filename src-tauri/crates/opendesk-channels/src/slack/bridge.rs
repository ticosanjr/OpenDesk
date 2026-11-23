//! Slack Socket Mode bridge.

use super::{
    client::connections_open,
    config::SlackConfig,
    protocol::{SlackEvent, ack, allowed, parse_envelope},
};
use crate::error::{AppError, AppResult};
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use tokio::{sync::Mutex, task::JoinHandle};
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub trait SlackEventHandler: Send + Sync {
    fn handle<'a>(
        &'a self,
        config: SlackConfig,
        event: SlackEvent,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>>;
}

#[derive(Default)]
pub struct SlackBridge {
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl SlackBridge {
    pub async fn is_running(&self) -> bool {
        self.handle
            .lock()
            .await
            .as_ref()
            .map(|handle| !handle.is_finished())
            .unwrap_or(false)
    }

    pub async fn stop(&self) {
        if let Some(handle) = self.handle.lock().await.take() {
            handle.abort();
        }
    }

    pub async fn start(
        self: &Arc<Self>,
        config: SlackConfig,
        app_token: String,
        handler: Arc<dyn SlackEventHandler>,
    ) {
        self.stop().await;
        let handle = tokio::spawn(async move {
            loop {
                if let Err(error) = run_connection(&config, &app_token, handler.as_ref()).await {
                    tracing::warn!(error = %error, "Slack Socket Mode disconnected");
                }
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
        *self.handle.lock().await = Some(handle);
    }
}

async fn run_connection(
    config: &SlackConfig,
    app_token: &str,
    handler: &dyn SlackEventHandler,
) -> AppResult<()> {
    let url = connections_open(app_token).await?;
    run_connection_at(&url, config, handler).await
}

async fn run_connection_at(
    url: &str,
    config: &SlackConfig,
    handler: &dyn SlackEventHandler,
) -> AppResult<()> {
    let (mut socket, _) = connect_async(url)
        .await
        .map_err(|error| AppError::Internal(format!("Slack connect: {error}")))?;
    loop {
        let Some(message) = socket.next().await else {
            return Ok(());
        };
        let message =
            message.map_err(|error| AppError::Internal(format!("Slack socket: {error}")))?;
        match message {
            Message::Text(text) => {
                let value: Value = serde_json::from_str(&text)
                    .map_err(|error| AppError::BadRequest(format!("Slack JSON: {error}")))?;
                match value
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                {
                    "hello" => {}
                    "disconnect" => return Ok(()),
                    "events_api" | "interactive" => {
                        if let Some(envelope_id) = value.get("envelope_id").and_then(Value::as_str)
                        {
                            socket
                                .send(Message::Text(ack(envelope_id).into()))
                                .await
                                .map_err(|error| {
                                    AppError::Internal(format!("Slack ack: {error}"))
                                })?;
                        }
                        if let Some(event) = parse_envelope(&value)
                            && allowed(config, &event)
                        {
                            handler.handle(config.clone(), event).await?;
                        }
                    }
                    _ => {}
                }
            }
            Message::Ping(payload) => {
                let _ = socket.send(Message::Pong(payload)).await;
            }
            Message::Close(_) => return Ok(()),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    struct Capture(Mutex<Vec<SlackEvent>>);

    impl SlackEventHandler for Capture {
        fn handle<'a>(
            &'a self,
            _config: SlackConfig,
            event: SlackEvent,
        ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
            Box::pin(async move {
                self.0.lock().expect("events").push(event);
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn acknowledges_and_dispatches_socket_mode_event() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut socket = accept_async(stream).await.expect("ws");
            socket
                .send(Message::Text(
                    serde_json::json!({ "type": "hello" }).to_string().into(),
                ))
                .await
                .expect("hello");
            socket
                .send(Message::Text(
                    serde_json::json!({
                        "type": "events_api",
                        "envelope_id": "e1",
                        "payload": {
                            "event": { "type": "message", "channel": "C1", "user": "U1", "text": "oi" }
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("event");
            let ack = socket.next().await.expect("ack").expect("ack message");
            if let Message::Text(text) = ack {
                assert!(text.contains("envelope_id"));
            }
            let _ = socket.close(None).await;
        });
        let capture = std::sync::Arc::new(Capture(Mutex::new(Vec::new())));
        let config = SlackConfig {
            channel_allowlist: vec!["C1".into()],
            ..Default::default()
        };
        run_connection_at(&format!("ws://{address}"), &config, capture.as_ref())
            .await
            .expect("connection");
        assert_eq!(capture.0.lock().expect("events").len(), 1);
    }
}
