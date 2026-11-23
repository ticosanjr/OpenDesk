//! Optional WhatsApp Web personal bridge (QR sidecar).
//!
//! The sidecar is a user-installed process speaking JSON Lines over stdio. The
//! bridge never bundles a Node/Baileys runtime and never restarts forever:
//! after five consecutive failures it opens the circuit and emits an error.

use crate::error::{AppError, AppResult};
use opendesk_storage::repos::settings;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use std::{future::Future, pin::Pin, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{Mutex, mpsc},
    task::JoinHandle,
};

pub const SETTING: &str = "whatsapp_qr";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppQrConfig {
    pub enabled: bool,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub sidecar_command: String,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default = "default_agent")]
    pub agent: String,
    #[serde(default)]
    pub notes: String,
}

fn default_agent() -> String {
    "build".into()
}

impl Default for WhatsAppQrConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            workspace_id: None,
            sidecar_command: String::new(),
            allowlist: Vec::new(),
            agent: default_agent(),
            notes: String::new(),
        }
    }
}

pub async fn load(pool: &SqlitePool) -> AppResult<WhatsAppQrConfig> {
    let value = settings::get(pool, SETTING).await?;
    Ok(value
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default())
}

pub async fn save(pool: &SqlitePool, config: &WhatsAppQrConfig) -> AppResult<()> {
    settings::set(
        pool,
        SETTING,
        &serde_json::to_value(config)
            .map_err(|error| AppError::Internal(format!("whatsapp qr config: {error}")))?,
    )
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub enum QrEvent {
    Qr(String),
    Status(String),
    Message {
        from: String,
        text: String,
        media: Vec<String>,
    },
    Error(String),
}

pub fn parse_line(line: &str) -> Option<QrEvent> {
    let value: Value = serde_json::from_str(line).ok()?;
    match value.get("type").and_then(Value::as_str)? {
        "qr" => value
            .get("qr")
            .and_then(Value::as_str)
            .map(|qr| QrEvent::Qr(qr.to_string())),
        "status" => value
            .get("status")
            .and_then(Value::as_str)
            .map(|status| QrEvent::Status(status.to_string())),
        "message" => Some(QrEvent::Message {
            from: value.get("from").and_then(Value::as_str)?.to_string(),
            text: value
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            media: value
                .get("media")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
        }),
        "error" => Some(QrEvent::Error(
            value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("erro do sidecar")
                .to_string(),
        )),
        _ => None,
    }
}

pub fn allowed(config: &WhatsAppQrConfig, from: &str) -> bool {
    config.allowlist.is_empty() || config.allowlist.iter().any(|item| item == from)
}

pub trait WhatsAppQrEventHandler: Send + Sync {
    fn handle<'a>(
        &'a self,
        config: WhatsAppQrConfig,
        event: QrEvent,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>>;
}

#[derive(Default)]
pub struct WhatsAppQrBridge {
    outbound: Mutex<Option<mpsc::Sender<String>>>,
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl WhatsAppQrBridge {
    pub async fn is_running(&self) -> bool {
        self.handle
            .lock()
            .await
            .as_ref()
            .map(|handle| !handle.is_finished())
            .unwrap_or(false)
    }

    pub async fn stop(&self) {
        *self.outbound.lock().await = None;
        if let Some(handle) = self.handle.lock().await.take() {
            handle.abort();
        }
    }

    pub async fn start(
        self: &Arc<Self>,
        config: WhatsAppQrConfig,
        handler: Arc<dyn WhatsAppQrEventHandler>,
    ) -> AppResult<()> {
        if config.sidecar_command.trim().is_empty() {
            return Err(AppError::BadRequest(
                "whatsapp_qr exige sidecar_command".into(),
            ));
        }
        self.stop().await;
        let (sender, receiver) = mpsc::channel(32);
        let handle = tokio::spawn(async move {
            supervise(config, handler, receiver).await;
        });
        *self.outbound.lock().await = Some(sender);
        *self.handle.lock().await = Some(handle);
        Ok(())
    }

    pub async fn send_text(&self, to: &str, text: &str) -> AppResult<()> {
        let sender = self
            .outbound
            .lock()
            .await
            .clone()
            .ok_or_else(|| AppError::Conflict("WhatsApp QR não está conectado".into()))?;
        sender
            .send(json!({ "type": "send", "to": to, "text": text }).to_string())
            .await
            .map_err(|_| AppError::Conflict("WhatsApp QR desconectado".into()))
    }
}

async fn supervise(
    config: WhatsAppQrConfig,
    handler: Arc<dyn WhatsAppQrEventHandler>,
    mut receiver: mpsc::Receiver<String>,
) {
    let mut failures = 0usize;
    loop {
        if failures >= 5 {
            let _ = handler
                .handle(config.clone(), QrEvent::Error("circuit_open".into()))
                .await;
            return;
        }
        match run_child(&config, handler.as_ref(), &mut receiver).await {
            Ok(()) => {
                let _ = handler
                    .handle(config.clone(), QrEvent::Status("disconnected".into()))
                    .await;
                return;
            }
            Err(error) => {
                failures += 1;
                let _ = handler
                    .handle(config.clone(), QrEvent::Error(format!("{error:#}")))
                    .await;
                tokio::time::sleep(Duration::from_secs((5 * failures.min(12)) as u64)).await;
            }
        }
    }
}

async fn run_child(
    config: &WhatsAppQrConfig,
    handler: &dyn WhatsAppQrEventHandler,
    receiver: &mut mpsc::Receiver<String>,
) -> AppResult<()> {
    #[cfg(unix)]
    let mut command = {
        let mut command = Command::new("sh");
        command.arg("-c").arg(&config.sidecar_command);
        command
    };
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("cmd");
        command.arg("/C").arg(&config.sidecar_command);
        command
    };
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| AppError::Internal(format!("sidecar WhatsApp: {error}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Internal("sidecar sem stdout".into()))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| AppError::Internal("sidecar sem stdin".into()))?;
    let mut lines = BufReader::new(stdout).lines();
    loop {
        tokio::select! {
            line = lines.next_line() => {
                let Some(line) = line.map_err(|error| AppError::Internal(format!("sidecar stdout: {error}")))? else {
                    return Ok(());
                };
                if let Some(event) = parse_line(&line) {
                    handler.handle(config.clone(), event).await?;
                }
            }
            outbound = receiver.recv() => {
                let Some(outbound) = outbound else { return Ok(()); };
                stdin.write_all(outbound.as_bytes()).await?;
                stdin.write_all(b"\n").await?;
                stdin.flush().await?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_qr_and_messages() {
        assert!(matches!(
            parse_line(r#"{"type":"qr","qr":"abc"}"#),
            Some(QrEvent::Qr(_))
        ));
        assert!(matches!(
            parse_line(r#"{"type":"message","from":"5511","text":"oi"}"#),
            Some(QrEvent::Message { .. })
        ));
        let config = WhatsAppQrConfig {
            allowlist: vec!["5511".into()],
            ..Default::default()
        };
        assert!(allowed(&config, "5511"));
        assert!(!allowed(&config, "5522"));
    }

    #[tokio::test]
    async fn starts_sidecar_reads_qr_and_sends_text() {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::Arc;
        use tokio::sync::mpsc;

        struct Forward(mpsc::Sender<QrEvent>);
        impl WhatsAppQrEventHandler for Forward {
            fn handle<'a>(
                &'a self,
                _config: WhatsAppQrConfig,
                event: QrEvent,
            ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>> {
                Box::pin(async move {
                    let _ = self.0.send(event).await;
                    Ok(())
                })
            }
        }

        let temp = tempfile::TempDir::new().expect("temp");
        let script = temp.path().join("bridge.sh");
        std::fs::write(
            &script,
            r#"#!/bin/sh
echo '{"type":"qr","qr":"data:image/png;base64,abc"}'
echo '{"type":"status","status":"connected"}'
while read line; do echo '{"type":"status","status":"sent"}'; done
"#,
        )
        .expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("mode");
        let (tx, mut rx) = mpsc::channel(8);
        let bridge = Arc::new(WhatsAppQrBridge::default());
        bridge
            .start(
                WhatsAppQrConfig {
                    enabled: true,
                    sidecar_command: script.to_string_lossy().to_string(),
                    ..Default::default()
                },
                Arc::new(Forward(tx)),
            )
            .await
            .expect("start");
        let first = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout")
            .expect("event");
        assert!(matches!(first, QrEvent::Qr(_)));
        let second = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout")
            .expect("event");
        assert!(matches!(second, QrEvent::Status(_)));
        bridge.send_text("5511", "oi").await.expect("send");
        let third = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout")
            .expect("event");
        assert!(matches!(third, QrEvent::Status(_)));
        bridge.stop().await;
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn saves_qr_config() {
        let temp = TempDir::new().expect("temp");
        let pool = opendesk_storage::init(&temp.path().join("db.sqlite"))
            .await
            .expect("db");
        let config = WhatsAppQrConfig {
            enabled: true,
            sidecar_command: "node x".into(),
            ..Default::default()
        };
        save(&pool, &config).await.expect("save");
        assert_eq!(load(&pool).await.expect("load").sidecar_command, "node x");
    }
}
