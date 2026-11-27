//! `signal-cli` daemon bridge and device linking.
//!
//! The bridge owns the long-lived child processes (`daemon` and `link`) but not
//! the routing of accepted messages: the host supplies a [`SignalEventHandler`]
//! so this module stays independent of `AppState`, chat and HITL.

use super::{
    config::{SignalConfig, detect_cli, load, signal_path},
    protocol::{IncomingSignal, allowed, parse_incoming},
};
use crate::error::{AppError, AppResult};
use sqlx::SqlitePool;
use std::{future::Future, pin::Pin, process::Stdio, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::Mutex,
};

/// Host callback invoked for every inbound message that passes the allowlist.
pub trait SignalEventHandler: Send + Sync {
    fn handle<'a>(
        &'a self,
        config: SignalConfig,
        incoming: IncomingSignal,
    ) -> Pin<Box<dyn Future<Output = AppResult<()>> + Send + 'a>>;
}

#[derive(Default)]
pub struct SignalBridge {
    handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    link_child: Mutex<Option<Child>>,
}

impl SignalBridge {
    pub async fn is_running(&self) -> bool {
        self.handle
            .lock()
            .await
            .as_ref()
            .map(|handle| !handle.is_finished())
            .unwrap_or(false)
    }

    pub async fn is_linking(&self) -> bool {
        let mut child = self.link_child.lock().await;
        match child.as_mut() {
            Some(value) => match value.try_wait() {
                Ok(Some(_)) => {
                    *child = None;
                    false
                }
                Ok(None) => true,
                Err(_) => {
                    *child = None;
                    false
                }
            },
            None => false,
        }
    }

    pub async fn stop(&self) {
        if let Some(handle) = self.handle.lock().await.take() {
            handle.abort();
        }
        let mut link = self.link_child.lock().await;
        if let Some(mut child) = link.take() {
            let _ = child.kill().await;
        }
    }

    pub async fn start_if_enabled(
        self: &Arc<Self>,
        pool: &SqlitePool,
        handler: Arc<dyn SignalEventHandler>,
    ) -> AppResult<bool> {
        self.stop().await;
        let config = load(pool).await?;
        if !config.enabled {
            return Ok(false);
        }
        if config.account.trim().is_empty() {
            return Err(AppError::BadRequest(
                "Signal está habilitado, mas a conta (número) não foi configurada.".into(),
            ));
        }
        if config.allowlist.is_empty() {
            return Err(AppError::BadRequest(
                "Signal está habilitado, mas a allowlist está vazia. Adicione ao menos um número."
                    .into(),
            ));
        }
        if detect_cli(&config.cli_path).await.is_none() {
            return Err(AppError::BadRequest(format!(
                "signal-cli não encontrado em '{}'. Instale ou informe o caminho.",
                config.cli_path
            )));
        }

        self.start_with_config(config, handler).await
    }

    /// Starts the bridge with an explicit configuration. Multi-bot workspaces
    /// use this entry point so each account gets its own process and handler.
    pub async fn start_with_config(
        self: &Arc<Self>,
        config: SignalConfig,
        handler: Arc<dyn SignalEventHandler>,
    ) -> AppResult<bool> {
        self.stop().await;
        if !config.enabled {
            return Ok(false);
        }
        if config.account.trim().is_empty() {
            return Err(AppError::BadRequest(
                "Signal está habilitado, mas a conta (número) não foi configurada.".into(),
            ));
        }
        if config.allowlist.is_empty() {
            return Err(AppError::BadRequest(
                "Signal está habilitado, mas a allowlist está vazia. Adicione ao menos um número."
                    .into(),
            ));
        }
        if detect_cli(&config.cli_path).await.is_none() {
            return Err(AppError::BadRequest(format!(
                "signal-cli não encontrado em '{}'. Instale ou informe o caminho.",
                config.cli_path
            )));
        }

        let bridge = Arc::clone(self);
        let handle = tokio::spawn(async move {
            if let Err(error) = bridge.run_with_config(config, handler).await {
                tracing::error!(error = %error, "Signal bridge stopped");
            }
        });
        *self.handle.lock().await = Some(handle);
        Ok(true)
    }

    async fn run_with_config(
        self: Arc<Self>,
        config: SignalConfig,
        handler: Arc<dyn SignalEventHandler>,
    ) -> anyhow::Result<()> {
        let mut command = Command::new(&config.cli_path);
        command
            .args(["-a", &config.account, "--output", "json", "daemon"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command
            .spawn()
            .map_err(|error| anyhow::anyhow!("could not start signal-cli: {error}"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("signal-cli stdout unavailable"))?;
        let mut lines = BufReader::new(stdout).lines();
        while let Some(line) = lines.next_line().await? {
            let Some(incoming) = parse_incoming(&line) else {
                continue;
            };
            if incoming.source == config.account {
                continue;
            }
            if !allowed(&config.allowlist, &incoming.source) {
                tracing::warn!(source = %incoming.source, "Signal message rejected by allowlist");
                continue;
            }
            if let Err(error) = handler.handle(config.clone(), incoming).await {
                tracing::warn!(error = %error, "Signal message failed");
            }
        }
        let _ = child.wait().await;
        Ok(())
    }

    /// Start `signal-cli link` and return the `sgnl://` URI to render as a QR
    /// code. The process stays alive so the phone can complete the link.
    pub async fn link(&self, cli_path: &str) -> AppResult<String> {
        let mut link = self.link_child.lock().await;
        if let Some(mut child) = link.take() {
            let _ = child.kill().await;
        }
        let mut command = Command::new(cli_path);
        command
            .args(["link", "-n", "OpenDesk"])
            .env("PATH", signal_path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn().map_err(|error| {
            AppError::BadRequest(format!("could not run signal-cli link: {error}"))
        })?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        if stdout.is_none() && stderr.is_none() {
            let _ = child.kill().await;
            return Err(AppError::Internal(
                "signal-cli link não expôs stdout/stderr".into(),
            ));
        }

        // signal-cli versions differ: some print the sgnl:// URI on stdout,
        // others on stderr. Read both concurrently and keep draining them so
        // the process can wait for the phone without blocking.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        if let Some(stdout) = stdout {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx.send(line);
                }
            });
        }
        if let Some(stderr) = stderr {
            let tx = tx.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = tx.send(line);
                }
            });
        }
        drop(tx);

        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut uri = None;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await {
                Ok(Some(line)) => {
                    if let Some(index) = line.find("sgnl://") {
                        uri = Some(line[index..].trim().to_string());
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => {}
            }
        }

        let Some(uri) = uri else {
            let _ = child.kill().await;
            return Err(AppError::BadRequest(
                "signal-cli não retornou um URI de vínculo. Execute `signal-cli link -n OpenDesk` no terminal e escaneie o QR."
                    .into(),
            ));
        };

        *link = Some(child);
        Ok(uri)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn link_extracts_the_signal_uri() {
        let temp = tempfile::TempDir::new().expect("temp");
        let script = temp.path().join("signal");
        std::fs::write(
            &script,
            "#!/bin/sh\necho 'sgnl://linkdevice?uuid=abc&id=xyz'\nsleep 5\n",
        )
        .expect("script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .expect("chmod");
        }
        let bridge = SignalBridge::default();
        let uri = bridge
            .link(&script.to_string_lossy())
            .await
            .expect("link uri");
        assert!(uri.starts_with("sgnl://linkdevice"));
        bridge.stop().await;
    }

    #[tokio::test]
    async fn link_reads_the_uri_from_stderr_too() {
        let temp = tempfile::TempDir::new().expect("temp");
        let script = temp.path().join("signal-stderr");
        std::fs::write(
            &script,
            "#!/bin/sh\necho 'sgnl://linkdevice?uuid=stderr&id=xyz' 1>&2\nsleep 5\n",
        )
        .expect("script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .expect("chmod");
        }
        let bridge = SignalBridge::default();
        let uri = bridge
            .link(&script.to_string_lossy())
            .await
            .expect("link uri");
        assert!(uri.starts_with("sgnl://linkdevice"));
        bridge.stop().await;
    }
}
