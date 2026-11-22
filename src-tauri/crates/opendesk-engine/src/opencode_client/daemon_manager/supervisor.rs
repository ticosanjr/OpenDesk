use super::*;

use anyhow::Context;
use std::process::Stdio;
use tokio::{process::Command, sync::mpsc, time::timeout};

impl DaemonManager {
    pub(super) async fn supervise(self: Arc<Self>) {
        let mut backoff = Duration::from_secs(1);

        loop {
            if self.inner.read().await.shutdown.is_cancelled() {
                break;
            }

            match self.ensure_running().await {
                Ok(()) => {
                    backoff = Duration::from_secs(2);
                    self.health_loop().await;
                }
                Err(error) => {
                    self.mark_offline(format!("{error:#}")).await;
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(Duration::from_secs(30));
                }
            }
        }

        self.stop_child().await;
    }

    async fn ensure_running(&self) -> Result<()> {
        {
            let inner = self.inner.read().await;
            if inner.child.is_some() && inner.status.online {
                return Ok(());
            }
        }

        if let Some(lock) = read_lock_file(&self.paths.daemon_lock_path) {
            let rest = RestClient::new(lock.port, lock.password.clone());
            if let Ok(info) = rest.info().await {
                tracing::info!(port = lock.port, version = %info.version, "attached to existing OpenCode daemon");
                let binary_path = self.locator.resolve().ok().map(|binary| binary.path);
                self.install_online(
                    lock.port,
                    lock.password,
                    rest,
                    Some(info.version),
                    None,
                    Some(lock.pid),
                    "dedicated",
                    binary_path,
                )
                .await;
                return Ok(());
            }

            if lock.pid > 0 {
                crate::process::terminate_pid(lock.pid).await;
            }
            let _ = std::fs::remove_file(&self.paths.daemon_lock_path);
        }

        let binary_error = match self.locator.resolve() {
            Ok(binary) => match self.spawn_dedicated(&binary).await {
                Ok(()) => return Ok(()),
                Err(error) => {
                    tracing::warn!(error = %error, "could not spawn a dedicated OpenCode daemon");
                    Some(format!("{error:#}"))
                }
            },
            Err(error) => {
                tracing::warn!(error = %error, "OpenCode binary not found");
                Some(format!("{error:#}"))
            }
        };

        match self.attach_shared_service().await {
            Ok(()) => Ok(()),
            Err(shared_error) => {
                let binary_message =
                    binary_error.unwrap_or_else(|| "no OpenCode binary was resolved".into());
                Err(anyhow!(
                    "{binary_message} Shared service fallback also failed: {shared_error:#}"
                ))
            }
        }
    }

    async fn spawn_dedicated(&self, binary: &OpenCodeBinaryInfo) -> Result<()> {
        let port = find_free_port()?;
        self.mark_state(
            "starting",
            Some(format!("starting {} serve on port {port}", binary.path)),
        )
        .await;

        let mut command = Command::new(&binary.path);
        command
            .args([
                "serve",
                "--hostname",
                "127.0.0.1",
                "--port",
                &port.to_string(),
            ])
            .env("PATH", enriched_path(&binary.path))
            .env("OPENDESK_DAEMON", "1")
            .env("OPENDESK_PARENT_PID", std::process::id().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        #[cfg(unix)]
        {
            command.process_group(0);
        }

        let mut child = command
            .spawn()
            .with_context(|| format!("could not start `{} serve`", binary.path))?;
        let pid = child.id().unwrap_or_default();

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("opencode stdout was not captured"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("opencode stderr was not captured"))?;

        let (line_tx, mut line_rx) = mpsc::channel::<String>(32);
        spawn_line_reader(stdout, "stdout", line_tx.clone());
        spawn_line_reader(stderr, "stderr", line_tx);

        let password = match timeout(STARTUP_TIMEOUT, async {
            while let Some(line) = line_rx.recv().await {
                if let Some(password) = line.strip_prefix("server password ") {
                    return Some(password.trim().to_string());
                }
            }
            None
        })
        .await
        {
            Ok(Some(password)) if !password.is_empty() => password,
            _ => {
                crate::process::terminate_child(&mut child, Duration::from_secs(3)).await;
                return Err(anyhow!(
                    "timed out waiting for the OpenCode server password on stdout/stderr"
                ));
            }
        };

        let rest = RestClient::new(port, password.clone());
        let mut info = None;
        for _ in 0..40 {
            match rest.info().await {
                Ok(value) => {
                    info = Some(value);
                    break;
                }
                Err(_) => tokio::time::sleep(Duration::from_millis(250)).await,
            }
        }

        let info = match info {
            Some(info) => info,
            None => {
                crate::process::terminate_child(&mut child, Duration::from_secs(3)).await;
                return Err(anyhow!(
                    "OpenCode server on port {port} did not become healthy with the captured password"
                ));
            }
        };

        write_lock_file(
            &self.paths.daemon_lock_path,
            &DaemonLock {
                pid,
                port,
                password: password.clone(),
            },
        )?;

        self.install_online(
            port,
            password,
            rest,
            Some(info.version),
            Some(child),
            Some(pid),
            "dedicated",
            Some(binary.path.clone()),
        )
        .await;

        Ok(())
    }

    async fn attach_shared_service(&self) -> Result<()> {
        for path in service_state_paths() {
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
                continue;
            };
            let Some(url) = value.get("url").and_then(|value| value.as_str()) else {
                continue;
            };
            let Some(password) = value.get("password").and_then(|value| value.as_str()) else {
                continue;
            };

            let rest = RestClient::new_with_url(url, password.to_string());
            if let Ok(info) = rest.info().await {
                if !info.version.starts_with('2') {
                    continue;
                }
                let port = reqwest::Url::parse(url)
                    .ok()
                    .and_then(|url| url.port())
                    .unwrap_or(DEFAULT_PORT);
                tracing::info!(url, version = %info.version, "attached to shared OpenCode service");
                self.install_online(
                    port,
                    password.to_string(),
                    rest,
                    Some(info.version),
                    None,
                    None,
                    "shared",
                    None,
                )
                .await;
                return Ok(());
            }
        }

        Err(anyhow!(
            "no healthy shared OpenCode service found in {:?}",
            service_state_paths()
        ))
    }

    #[allow(clippy::too_many_arguments)]
    async fn install_online(
        &self,
        port: u16,
        password: String,
        rest: RestClient,
        version: Option<String>,
        child: Option<Child>,
        pid: Option<u32>,
        mode: &str,
        binary_path: Option<String>,
    ) {
        let status = {
            let mut inner = self.inner.write().await;
            inner.child = child;
            inner.pid = pid;
            inner.password = Some(password);
            inner.rest = Some(rest);
            inner.status = DaemonStatus {
                online: true,
                port,
                active_jobs: inner.status.active_jobs,
                state: "online".into(),
                message: None,
                version,
                mode: mode.to_string(),
                binary_path,
            };
            inner.status.clone()
        };
        self.publish(status);
    }

    async fn health_loop(self: &Arc<Self>) {
        let mut failures: u8 = 0;

        loop {
            tokio::time::sleep(HEALTH_INTERVAL).await;

            if self.inner.read().await.shutdown.is_cancelled() {
                return;
            }

            let rest = {
                let inner = self.inner.read().await;
                inner.rest.clone()
            };
            let Some(rest) = rest else {
                return;
            };

            match rest.info().await {
                Ok(info) => {
                    failures = 0;
                    let should_publish = {
                        let mut inner = self.inner.write().await;
                        let was_online = inner.status.online;
                        inner.status.online = true;
                        inner.status.state = "online".into();
                        inner.status.message = None;
                        inner.status.version = Some(info.version);
                        !was_online
                    };
                    if should_publish {
                        self.publish_status().await;
                    }
                }
                Err(error) => {
                    failures += 1;
                    tracing::warn!(failures, error = %error, "OpenCode healthcheck failed");
                    if failures >= MAX_HEALTH_FAILURES {
                        self.mark_offline(format!("healthcheck failed: {error}"))
                            .await;
                        self.stop_child().await;
                        return;
                    }
                }
            }
        }
    }

    pub(super) async fn mark_state(&self, state: &str, message: Option<String>) {
        let status = {
            let mut inner = self.inner.write().await;
            inner.status.state = state.to_string();
            inner.status.message = message;
            if state != "online" {
                inner.status.online = false;
            }
            inner.status.clone()
        };
        self.publish(status);
    }

    pub(super) async fn mark_offline(&self, message: String) {
        let status = {
            let mut inner = self.inner.write().await;
            inner.status.online = false;
            inner.status.state = "offline".into();
            inner.status.mode = "offline".into();
            inner.status.message = Some(message);
            inner.rest = None;
            inner.status.clone()
        };
        self.publish(status);
    }

    pub(super) async fn publish_status(&self) {
        let status = self.inner.read().await.status.clone();
        self.publish(status);
    }

    fn publish(&self, status: DaemonStatus) {
        self.events.publish(OpenDeskWsEvent::DaemonStatusChanged {
            online: status.online,
            port: status.port,
            active_jobs: status.active_jobs,
            state: status.state,
            message: status.message,
            version: status.version,
            mode: status.mode,
            binary_path: status.binary_path,
        });
    }

    pub(super) async fn stop_child(&self) {
        let (child, pid, port, mode) = {
            let mut inner = self.inner.write().await;
            inner.rest = None;
            inner.password = None;
            (
                inner.child.take(),
                inner.pid.take(),
                inner.status.port,
                inner.status.mode.clone(),
            )
        };

        if let Some(mut child) = child {
            crate::process::terminate_child(&mut child, Duration::from_secs(5)).await;
        } else if let Some(pid) = pid {
            crate::process::terminate_pid(pid).await;
        }

        // A shared service owns its port; waiting for it to be free would just
        // delay the restart by three seconds. Only wait for a dedicated child.
        if mode != "shared" {
            for _ in 0..30 {
                if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        let _ = std::fs::remove_file(&self.paths.daemon_lock_path);
    }
}
