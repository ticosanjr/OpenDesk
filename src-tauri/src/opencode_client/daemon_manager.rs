use crate::{
    events::{EventBus, OpenDeskWsEvent},
    opencode_client::{
        binary::{OpenCodeBinaryInfo, OpenCodeLocator},
        rest_client::{AgentDto, ModelDto, RestClient},
    },
    paths::AppPaths,
};
use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::{Child, Command},
    sync::{RwLock, mpsc},
    time::timeout,
};
use tokio_util::sync::CancellationToken;

const DEFAULT_PORT: u16 = 3100;
const LAST_PORT: u16 = 3199;
const STARTUP_TIMEOUT: Duration = Duration::from_secs(20);
const HEALTH_INTERVAL: Duration = Duration::from_secs(5);
const MAX_HEALTH_FAILURES: u8 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DaemonStatus {
    pub online: bool,
    pub port: u16,
    pub active_jobs: usize,
    pub state: String,
    pub message: Option<String>,
    pub version: Option<String>,
    pub mode: String,
    pub binary_path: Option<String>,
}

struct Inner {
    status: DaemonStatus,
    child: Option<Child>,
    pid: Option<u32>,
    password: Option<String>,
    rest: Option<RestClient>,
    shutdown: CancellationToken,
}

pub struct DaemonManager {
    paths: AppPaths,
    events: EventBus,
    locator: Arc<OpenCodeLocator>,
    inner: RwLock<Inner>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DaemonLock {
    pid: u32,
    port: u16,
    password: String,
}

impl DaemonManager {
    pub fn new(paths: AppPaths, events: EventBus, locator: Arc<OpenCodeLocator>) -> Arc<Self> {
        Arc::new(Self {
            paths,
            events,
            locator,
            inner: RwLock::new(Inner {
                status: DaemonStatus {
                    online: false,
                    port: DEFAULT_PORT,
                    active_jobs: 0,
                    state: "starting".into(),
                    message: None,
                    version: None,
                    mode: "offline".into(),
                    binary_path: None,
                },
                child: None,
                pid: None,
                password: None,
                rest: None,
                shutdown: CancellationToken::new(),
            }),
        })
    }

    pub fn locator(&self) -> Arc<OpenCodeLocator> {
        Arc::clone(&self.locator)
    }

    pub fn spawn_supervisor(self: &Arc<Self>) {
        let manager = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            manager.supervise().await;
        });
    }

    pub async fn status(&self) -> DaemonStatus {
        self.inner.read().await.status.clone()
    }

    pub async fn client(&self) -> Option<RestClient> {
        self.inner.read().await.rest.clone()
    }

    pub async fn models(&self, directory: Option<&str>) -> Result<Vec<ModelDto>> {
        let client = self
            .client()
            .await
            .ok_or_else(|| anyhow!("OpenCode daemon is not online"))?;
        client.models(directory).await
    }

    pub async fn agents(&self, directory: Option<&str>) -> Result<Vec<AgentDto>> {
        let client = self
            .client()
            .await
            .ok_or_else(|| anyhow!("OpenCode daemon is not online"))?;
        client.agents(directory).await
    }

    #[allow(dead_code)]
    pub async fn set_active_jobs(&self, active_jobs: usize) {
        let changed = {
            let mut inner = self.inner.write().await;
            if inner.status.active_jobs == active_jobs {
                false
            } else {
                inner.status.active_jobs = active_jobs;
                true
            }
        };
        if changed {
            self.publish_status().await;
        }
    }

    pub async fn restart(&self) {
        self.mark_state("restarting", Some("restart requested".into()))
            .await;
        self.stop_child().await;
    }

    /// Wait until the supervisor reports a terminal state after a restart.
    /// Used by the engine API so the UI can show the real outcome instead of
    /// an optimistic "restarting" that never resolves.
    pub async fn wait_for_settled(&self, timeout: Duration) -> DaemonStatus {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let status = self.status().await;
            let settled = status.online || matches!(status.state.as_str(), "offline" | "online");
            if settled || tokio::time::Instant::now() >= deadline {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    pub async fn shutdown(&self) {
        self.inner.read().await.shutdown.cancel();
        self.stop_child().await;
    }

    /// Used by headless mode when the daemon is intentionally disabled.
    pub async fn mark_unavailable(&self, message: &str) {
        self.mark_offline(message.to_string()).await;
    }

    async fn supervise(self: Arc<Self>) {
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
                terminate_pid(lock.pid).await;
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
            Ok(()) => return Ok(()),
            Err(shared_error) => {
                let binary_message =
                    binary_error.unwrap_or_else(|| "no OpenCode binary was resolved".into());
                return Err(anyhow!(
                    "{binary_message} Shared service fallback also failed: {shared_error:#}"
                ));
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
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

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
                let _ = child.kill().await;
                let _ = child.wait().await;
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
                let _ = child.kill().await;
                let _ = child.wait().await;
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

    async fn mark_state(&self, state: &str, message: Option<String>) {
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

    async fn mark_offline(&self, message: String) {
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

    async fn publish_status(&self) {
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

    async fn stop_child(&self) {
        let (child, pid, port) = {
            let mut inner = self.inner.write().await;
            inner.rest = None;
            inner.password = None;
            (inner.child.take(), inner.pid.take(), inner.status.port)
        };

        if let Some(mut child) = child {
            let _ = child.kill().await;
            let _ = child.wait().await;
        } else if let Some(pid) = pid {
            terminate_pid(pid).await;
        }

        // Give the OS a moment to release the port before the supervisor
        // attempts to spawn a replacement.
        for _ in 0..30 {
            if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let _ = std::fs::remove_file(&self.paths.daemon_lock_path);
    }
}

fn spawn_line_reader<R>(reader: R, source: &'static str, line_tx: mpsc::Sender<String>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if line.starts_with("server password ") {
                        let _ = line_tx.send(line).await;
                    } else {
                        tracing::debug!(target: "opencode", source, "{line}");
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    tracing::debug!(target: "opencode", source, error = %error, "log reader stopped");
                    break;
                }
            }
        }
    });
}

async fn terminate_pid(pid: u32) {
    #[cfg(unix)]
    {
        let _ = tokio::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .await;
    }

    #[cfg(windows)]
    {
        let _ = tokio::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()
            .await;
    }
}

fn enriched_path(binary: &str) -> String {
    let mut paths = Vec::new();
    if let Some(parent) = std::path::Path::new(binary).parent() {
        paths.push(parent.to_path_buf());
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        paths.push(home.join(".opencode/bin"));
        paths.push(home.join(".local/bin"));
        paths.push(home.join(".bun/bin"));
    }
    #[cfg(windows)]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        paths.push(PathBuf::from(appdata).join("npm"));
    }
    if let Some(current) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&current));
    }
    paths.push(PathBuf::from("/usr/local/bin"));
    paths.push(PathBuf::from("/usr/bin"));
    paths.push(PathBuf::from("/bin"));
    std::env::join_paths(paths)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| binary.to_string())
}

fn service_state_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(state) = std::env::var_os("XDG_STATE_HOME") {
        paths.push(PathBuf::from(state).join("opencode/service.json"));
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        paths.push(home.join(".local/state/opencode/service.json"));
        paths.push(home.join(".config/opencode/service.json"));
    }
    paths
}

fn find_free_port() -> Result<u16> {
    for port in DEFAULT_PORT..=LAST_PORT {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Ok(port);
        }
    }
    Err(anyhow!(
        "no free port in {DEFAULT_PORT}-{LAST_PORT} for the OpenCode daemon"
    ))
}

fn read_lock_file(path: &std::path::Path) -> Option<DaemonLock> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_lock_file(path: &std::path::Path, lock: &DaemonLock) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(lock)?;
    std::fs::write(path, bytes)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}
