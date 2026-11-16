use crate::{
    config_sync::{paths::StudioPaths, watcher::ConfigWatcher},
    db::{self, settings, workspaces},
    error::AppResult,
    events::EventBus,
    integrations::{google_oauth::GoogleOAuth, signal::SignalBridge, telegram::TelegramBridge},
    opencode_client::{binary::OpenCodeLocator, daemon_manager::DaemonManager},
    paths::AppPaths,
    queue_scheduler::{CronEngine, QueueConfig, QueueManager},
    secrets::SecretStore,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::{
    collections::HashMap,
    net::TcpListener,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// App setting that stores the user-chosen OpenCode binary path. It is
/// persisted so the AppImage keeps working across restarts even when the
/// binary directory is not on the inherited PATH.
pub const OPENCODE_BIN_SETTING: &str = "opencode_bin_path";
/// Number of queue jobs that may run concurrently (1–4).
pub const QUEUE_CONCURRENCY_SETTING: &str = "queue_concurrency";

#[derive(Debug, Clone)]
pub struct ActiveChatTurn {
    pub turn_id: String,
    pub session_id: Option<String>,
    pub cancel: CancellationToken,
}

pub struct AppState {
    pub paths: AppPaths,
    pub db: SqlitePool,
    pub events: EventBus,
    pub write_buffer: Arc<crate::db::write_buffer::WriteBuffer>,
    pub daemon: Arc<DaemonManager>,
    pub locator: Arc<OpenCodeLocator>,
    pub queue: Arc<QueueManager>,
    pub cron: Arc<CronEngine>,
    pub secrets: Arc<SecretStore>,
    pub google: Arc<GoogleOAuth>,
    pub signal: Arc<SignalBridge>,
    pub telegram: Arc<TelegramBridge>,
    pub io_bus: Arc<crate::io_bus::runtime::IoBus>,
    pub push_keys: crate::push::VapidKeys,
    pub adapters: Arc<crate::adapters::AdapterService>,
    pub rate_limiter: crate::audit::RateLimiter,
    pub api_host: String,
    pub api_port: u16,
    pub api_token: String,
    pub listener: Mutex<Option<TcpListener>>,
    pub studio_watcher: Mutex<ConfigWatcher>,
    pub started_at: DateTime<Utc>,
    chat_turns: Mutex<HashMap<String, ActiveChatTurn>>,
}

pub use crate::api_endpoint::ApiEndpoint;

impl AppState {
    pub async fn initialize(paths: AppPaths) -> Result<Self> {
        let db = db::init(&paths.db_path).await?;
        db::maintenance::boot_check(&db).await?;
        if let Err(error) = db::worktrees::sweep(&db, &paths.data_dir).await {
            tracing::warn!(error = %error, "worktree sweep failed");
        }

        if settings::get(&db, "workspaces_root").await?.is_none() {
            let root = crate::paths::default_workspaces_root();
            std::fs::create_dir_all(&root)
                .with_context(|| format!("could not create {}", root.display()))?;
            settings::set(
                &db,
                "workspaces_root",
                &serde_json::Value::String(root.to_string_lossy().to_string()),
            )
            .await?;
        }

        let requested_port = std::env::var("OPENDESK_API_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0);
        let api_host = std::env::var("OPENDESK_API_HOST")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "127.0.0.1".into());
        let listener = TcpListener::bind((api_host.as_str(), requested_port))
            .context("could not reserve a local port for the OpenDesk API")?;
        listener
            .set_nonblocking(true)
            .context("could not configure the OpenDesk API listener")?;
        let api_port = listener
            .local_addr()
            .context("could not read the OpenDesk API port")?
            .port();

        let api_token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());

        let events = EventBus::new(2048);
        let write_buffer = crate::db::write_buffer::WriteBuffer::start(
            db.clone(),
            std::time::Duration::from_millis(250),
            1024,
        );
        let push_keys = crate::push::load_or_create_keys(&paths)?;
        let configured_binary = settings::get(&db, OPENCODE_BIN_SETTING)
            .await?
            .and_then(|value| value.as_str().map(PathBuf::from));
        let locator = Arc::new(OpenCodeLocator::new(configured_binary));
        let daemon = DaemonManager::new(paths.clone(), events.clone(), locator.clone());
        let secrets = Arc::new(SecretStore::new(&paths));
        let adapters = Arc::new(crate::adapters::AdapterService::load(&db).await?);

        let google = GoogleOAuth::new(db.clone(), secrets.clone(), events.clone());
        let signal = Arc::new(SignalBridge::default());
        let telegram = Arc::new(TelegramBridge::default());
        let io_bus = Arc::new(crate::io_bus::runtime::IoBus::new());
        let concurrency = settings::get(&db, QUEUE_CONCURRENCY_SETTING)
            .await?
            .and_then(|value| value.as_u64())
            .unwrap_or(1) as usize;
        let queue = QueueManager::new(QueueConfig {
            pool: db.clone(),
            paths: paths.clone(),
            events: events.clone(),
            write_buffer: write_buffer.clone(),
            daemon: daemon.clone(),
            google: google.clone(),
            concurrency,
            adapters: adapters.clone(),
        });
        let cron = CronEngine::new(
            db.clone(),
            paths.clone(),
            events.clone(),
            queue.clone(),
            daemon.clone(),
        );

        let state = Self {
            paths,
            db,
            events,
            write_buffer,
            daemon,
            locator,
            queue,
            cron,
            secrets,
            google,
            signal,
            telegram,
            io_bus,
            push_keys,
            adapters,
            rate_limiter: crate::audit::RateLimiter::new(crate::audit::RateLimits::from_env()),
            api_host,
            api_port,
            api_token,
            listener: Mutex::new(Some(listener)),
            studio_watcher: Mutex::new(ConfigWatcher::new()),
            started_at: Utc::now(),
            chat_turns: Mutex::new(HashMap::new()),
        };

        state.write_api_lock()?;
        Ok(state)
    }

    /// Registers the active turn for a chat thread. The returned token lets the
    /// wait loop stop immediately when the user presses Parar.
    pub fn begin_chat_turn(
        &self,
        thread_id: &str,
        turn_id: &str,
        session_id: Option<&str>,
    ) -> CancellationToken {
        let cancel = CancellationToken::new();
        self.chat_turns
            .lock()
            .expect("chat turn mutex is not poisoned")
            .insert(
                thread_id.to_string(),
                ActiveChatTurn {
                    turn_id: turn_id.to_string(),
                    session_id: session_id.map(str::to_string),
                    cancel: cancel.clone(),
                },
            );
        cancel
    }

    pub fn finish_chat_turn(&self, thread_id: &str, turn_id: &str) {
        let mut turns = self
            .chat_turns
            .lock()
            .expect("chat turn mutex is not poisoned");
        if turns
            .get(thread_id)
            .is_some_and(|turn| turn.turn_id == turn_id)
        {
            turns.remove(thread_id);
        }
    }

    pub fn active_chat_turn(&self, thread_id: &str) -> Option<String> {
        self.chat_turns
            .lock()
            .expect("chat turn mutex is not poisoned")
            .get(thread_id)
            .map(|turn| turn.turn_id.clone())
    }

    /// Cancels the active turn and returns its session so the caller can also
    /// interrupt the OpenCode execution.
    pub fn cancel_chat_turn(&self, thread_id: &str) -> Option<(String, Option<String>)> {
        let turns = self
            .chat_turns
            .lock()
            .expect("chat turn mutex is not poisoned");
        let turn = turns.get(thread_id)?;
        turn.cancel.cancel();
        Some((turn.turn_id.clone(), turn.session_id.clone()))
    }

    pub fn take_listener(&self) -> Option<TcpListener> {
        self.listener
            .lock()
            .expect("OpenDesk listener mutex is not poisoned")
            .take()
    }

    pub fn api_endpoint(&self) -> ApiEndpoint {
        ApiEndpoint {
            host: self.api_host.clone(),
            port: self.api_port,
            token: self.api_token.clone(),
        }
    }

    /// Rebuild the watcher for the active workspace (or just the global scope
    /// when no workspace is selected). Called on boot and on workspace switch.
    pub async fn refresh_watcher(&self) -> AppResult<()> {
        let active = settings::active_workspace_id(&self.db).await?;
        let (paths, workspace_id) = match active {
            Some(id) => {
                let workspace = workspaces::get(&self.db, &id).await?;
                (
                    StudioPaths::new(Some(PathBuf::from(workspace.root_path))),
                    id,
                )
            }
            None => (StudioPaths::new(None), "global".to_string()),
        };

        let mut watcher = self
            .studio_watcher
            .lock()
            .expect("OpenDesk watcher mutex is not poisoned");
        watcher.watch(&paths, &workspace_id, self.events.clone())?;
        Ok(())
    }

    fn write_api_lock(&self) -> Result<()> {
        let path = &self.paths.api_lock_path;
        let bytes = serde_json::to_vec_pretty(&self.api_endpoint())?;
        std::fs::write(path, bytes)
            .with_context(|| format!("could not write {}", path.display()))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .with_context(|| format!("could not protect {}", path.display()))?;
        }

        Ok(())
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.paths.api_lock_path);
    }
}
