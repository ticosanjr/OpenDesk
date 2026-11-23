//! Discord gateway bridge.
//!
//! The bridge owns the WebSocket lifecycle and heartbeat; the host supplies a
//! handler for messages and interactions so routing stays in the shell.

use super::{
    config::DiscordConfig,
    protocol::{DiscordEvent, allowed, parse_dispatch},
};
use crate::error::{AppError, AppResult};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use tokio::{sync::Mutex, task::JoinHandle};
use tokio_tungstenite::{connect_async, tungstenite::Message};

fn gateway_url() -> String {
    std::env::var("OPENDESK_DISCORD_GATEWAY")
        .unwrap_or_else(|_| "wss://gateway.discord.gg/?v=10&encoding=json".into())
}

pub trait DiscordEventHandler: Send + Sync {
    fn handle<'a>(
        &'a self,
        config: DiscordConfig,
        event: DiscordEvent,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>>;
}

#[derive(Default)]
pub struct DiscordBridge {
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl DiscordBridge {
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
        config: DiscordConfig,
        token: String,
        handler: Arc<dyn DiscordEventHandler>,
    ) {
        self.stop().await;
        let handle = tokio::spawn(async move {
            loop {
                if let Err(error) = run_connection(&config, &token, handler.as_ref()).await {
                    tracing::warn!(error = %error, "Discord gateway disconnected");
                }
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
        *self.handle.lock().await = Some(handle);
    }
}

async fn run_connection(
    config: &DiscordConfig,
    token: &str,
    handler: &dyn DiscordEventHandler,
) -> AppResult<()> {
    run_connection_at(&gateway_url(), config, token, handler).await
}

async fn run_connection_at(
    url: &str,
    config: &DiscordConfig,
    token: &str,
    handler: &dyn DiscordEventHandler,
) -> AppResult<()> {
    let (mut socket, _) = connect_async(url)
        .await
        .map_err(|error| AppError::Internal(format!("Discord connect: {error}")))?;
    let hello = socket
        .next()
        .await
        .ok_or_else(|| AppError::Internal("Discord sem HELLO".into()))?
        .map_err(|error| AppError::Internal(format!("Discord hello: {error}")))?;
    let hello: Value = parse_text(&hello)?;
    let interval = hello
        .pointer("/d/heartbeat_interval")
        .and_then(Value::as_u64)
        .unwrap_or(41_250)
        .max(1_000);
    socket
        .send(Message::Text(
            json!({
                "op": 2,
                "d": {
                    "token": token,
                    "intents": (1 << 9) | (1 << 12) | (1 << 15),
                    "properties": { "os": "linux", "browser": "opendesk", "device": "opendesk" }
                }
            })
            .to_string()
            .into(),
        ))
        .await
        .map_err(|error| AppError::Internal(format!("Discord identify: {error}")))?;

    let mut heartbeat = tokio::time::interval(Duration::from_millis(interval));
    heartbeat.tick().await;
    let mut sequence = Value::Null;
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                socket
                    .send(Message::Text(json!({ "op": 1, "d": sequence }).to_string().into()))
                    .await
                    .map_err(|error| AppError::Internal(format!("Discord heartbeat: {error}")))?;
            }
            message = socket.next() => {
                let Some(message) = message else {
                    return Ok(());
                };
                let message = message.map_err(|error| AppError::Internal(format!("Discord socket: {error}")))?;
                match message {
                    Message::Text(text) => {
                        let value: Value = serde_json::from_str(&text)
                            .map_err(|error| AppError::BadRequest(format!("Discord JSON: {error}")))?;
                        if let Some(seq) = value.get("s") {
                            sequence = seq.clone();
                        }
                        match value.get("op").and_then(Value::as_i64) {
                            Some(7) | Some(9) => return Err(AppError::Internal("Discord pediu reconexão".into())),
                            Some(0) => {
                                if let Some(event) = parse_dispatch(&value) {
                                    let permitted = match &event {
                                        DiscordEvent::Message(message) => allowed(config, message),
                                        DiscordEvent::Interaction(interaction) => {
                                            (config.user_allowlist.is_empty()
                                                || config.user_allowlist.contains(&interaction.user_id))
                                                && (config.channel_allowlist.is_empty()
                                                    || config.channel_allowlist.contains(&interaction.channel_id))
                                        }
                                    };
                                    if permitted {
                                        handler.handle(config.clone(), event).await?;
                                    }
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
    }
}

fn parse_text(message: &Message) -> AppResult<Value> {
    let Message::Text(text) = message else {
        return Err(AppError::Internal(
            "Discord enviou mensagem não textual".into(),
        ));
    };
    serde_json::from_str(text)
        .map_err(|error| AppError::BadRequest(format!("Discord JSON: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    struct Capture(Mutex<Vec<DiscordEvent>>);

    impl DiscordEventHandler for Capture {
        fn handle<'a>(
            &'a self,
            _config: DiscordConfig,
            event: DiscordEvent,
        ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
            Box::pin(async move {
                self.0.lock().expect("events").push(event);
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn reads_gateway_message_and_dispatches_allowed_event() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept");
            let mut socket = accept_async(stream).await.expect("ws");
            socket
                .send(Message::Text(
                    json!({ "op": 10, "d": { "heartbeat_interval": 60000 } })
                        .to_string()
                        .into(),
                ))
                .await
                .expect("hello");
            let _ = socket.next().await;
            socket
                .send(Message::Text(
                    json!({
                        "op": 0,
                        "t": "MESSAGE_CREATE",
                        "d": {
                            "id": "m1",
                            "channel_id": "c1",
                            "content": "oi",
                            "author": { "id": "u1", "username": "ana", "bot": false }
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("message");
            let _ = socket.close(None).await;
        });
        let capture = std::sync::Arc::new(Capture(Mutex::new(Vec::new())));
        let config = DiscordConfig {
            channel_allowlist: vec!["c1".into()],
            ..Default::default()
        };
        run_connection_at(
            &format!("ws://{address}"),
            &config,
            "token",
            capture.as_ref(),
        )
        .await
        .expect("connection");
        assert_eq!(capture.0.lock().expect("events").len(), 1);
    }
}
