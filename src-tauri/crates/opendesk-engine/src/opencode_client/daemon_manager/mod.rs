mod helpers;
mod supervisor;

pub use helpers::cleanup_orphaned_daemons;
use helpers::*;

use crate::opencode_client::{
    binary::{OpenCodeBinaryInfo, OpenCodeLocator},
    rest_client::{AgentDto, ModelDto, RestClient},
};
use opendesk_storage::events::{EventBus, OpenDeskWsEvent};
use opendesk_storage::paths::AppPaths;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tokio::{process::Child, sync::RwLock};
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

    pub fn spawn_supervisor(self: &Arc<Self>) {
        cleanup_orphaned_daemons();
        let manager = Arc::clone(self);
        tokio::spawn(async move {
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
}

#[cfg(test)]
mod tests;
