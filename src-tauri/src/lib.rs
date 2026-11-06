mod api;
mod app_state;
mod chat;
mod config_sync;
mod db;
mod doctor;
mod dsh_router;
mod error;
mod events;
mod integrations;
mod mcp_bridge;
mod opencode_client;
mod paths;
mod queue_scheduler;
mod secrets;
mod tray;
mod updates;

use app_state::AppState;
use paths::AppPaths;
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Manager};
use tracing_subscriber::EnvFilter;

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

/// Tauri does not translate Unix SIGTERM/SIGINT into RunEvent::Exit. Handle
/// them explicitly so the managed OpenCode child is not orphaned on logout,
/// IDE restarts or `kill`.
fn install_shutdown_handlers(app: AppHandle, state: Arc<AppState>) {
    #[cfg(unix)]
    {
        tauri::async_runtime::spawn(async move {
            use tokio::signal::unix::{SignalKind, signal};

            let mut terminate = match signal(SignalKind::terminate()) {
                Ok(signal) => signal,
                Err(error) => {
                    tracing::warn!(error = %error, "could not install SIGTERM handler");
                    return;
                }
            };
            let mut interrupt = match signal(SignalKind::interrupt()) {
                Ok(signal) => signal,
                Err(error) => {
                    tracing::warn!(error = %error, "could not install SIGINT handler");
                    return;
                }
            };

            tokio::select! {
                _ = terminate.recv() => tracing::info!("SIGTERM received; shutting down"),
                _ = interrupt.recv() => tracing::info!("SIGINT received; shutting down"),
            }

            state.cron.shutdown();
            state.queue.shutdown().await;
            state.signal.stop().await;
            state.daemon.shutdown().await;
            app.exit(0);
        });
    }

    #[cfg(not(unix))]
    {
        tauri::async_runtime::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("shutdown signal received");
            state.cron.shutdown();
            state.queue.shutdown().await;
            state.signal.stop().await;
            state.daemon.shutdown().await;
            app.exit(0);
        });
    }
}

fn init_logging() {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("opendesk=debug,opencode=info,tower_http=info"));

    // Mirror logs to a rotating file so "why is it offline?" can be answered
    // after the fact from Settings → Diagnostics. Terminal output stays enabled
    // for AppImage launchers.
    let file_layer = match AppPaths::discover() {
        Ok(paths) => {
            let appender = tracing_appender::rolling::daily(&paths.log_dir, "opendesk.log");
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let _ = LOG_GUARD.set(guard);
            Some(
                tracing_subscriber::fmt::layer()
                    .with_target(true)
                    .with_writer(writer),
            )
        }
        Err(_) => None,
    };

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .with(file_layer)
        .try_init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }));

    let app = builder
        .setup(|app| {
            let paths = AppPaths::discover()?;
            let state = tauri::async_runtime::block_on(AppState::initialize(paths))?;
            let state = Arc::new(state);

            app.manage(state.clone());
            state.daemon.spawn_supervisor();
            tauri::async_runtime::spawn(integrations::delivery::retry_loop(state.clone()));
            if let Err(error) =
                tauri::async_runtime::block_on(state.signal.start_if_enabled(state.clone()))
            {
                tracing::warn!(error = %error, "Signal bridge was not started");
            }
            state.queue.spawn_worker();
            state.cron.spawn();
            tray::setup(app.handle(), state.clone())?;
            tauri::async_runtime::block_on(state.refresh_watcher())?;
            tauri::async_runtime::block_on(api::spawn_server(state.clone()))?;

            install_shutdown_handlers(app.handle().clone(), state.clone());

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            api::commands::get_api_endpoint,
            api::commands::get_daemon_status
        ])
        .build(tauri::generate_context!())
        .expect("error while building the OpenDesk application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event
            && let Some(state) = app_handle.try_state::<Arc<AppState>>()
        {
            let state = state.inner().clone();
            tauri::async_runtime::block_on(async {
                state.cron.shutdown();
                state.queue.shutdown().await;
                state.daemon.shutdown().await;
            });
        }
    });
}

/// Headless mode used by browser tests and future daemon-only installs.
/// It runs the same AppState, SQLite, watcher and Axum API without a Tauri
/// window or tray. `OPENDESK_SKIP_DAEMON=1` keeps the OpenCode engine offline.
pub fn run_headless() {
    init_logging();

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let paths = AppPaths::discover().expect("application paths");
        let state = Arc::new(
            AppState::initialize(paths)
                .await
                .expect("initialize OpenDesk state"),
        );

        if std::env::var("OPENDESK_SKIP_DAEMON").is_ok() {
            state
                .daemon
                .mark_unavailable("OpenCode daemon disabled by OPENDESK_SKIP_DAEMON")
                .await;
        } else {
            state.daemon.spawn_supervisor();
            tauri::async_runtime::spawn(integrations::delivery::retry_loop(state.clone()));
        }

        state.refresh_watcher().await.expect("config watcher");
        if let Err(error) = state.signal.start_if_enabled(state.clone()).await {
            tracing::warn!(error = %error, "Signal bridge was not started");
        }
        state.queue.spawn_worker();
        tauri::async_runtime::spawn(integrations::delivery::retry_loop(state.clone()));
        state.cron.spawn();
        api::spawn_server(state.clone())
            .await
            .expect("OpenDesk API server");

        println!(
            "OpenDesk headless API on http://127.0.0.1:{} (token in api.json)",
            state.api_port
        );

        let _ = tokio::signal::ctrl_c().await;
        state.cron.shutdown();
        state.queue.shutdown().await;
        state.daemon.shutdown().await;
    });
}

/// `opendesk doctor` — preflight for the engine and local state without a GUI.
pub fn run_doctor(json: bool, fix_daemons: bool) {
    std::process::exit(doctor::run(json, fix_daemons));
}

/// MCP server over stdio used by OpenCode's `mcp.servers.opendesk-google`.
/// Stdout is reserved for JSON-RPC frames; diagnostics go to stderr.
pub fn run_mcp_google() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    if let Err(error) = runtime.block_on(mcp_bridge::run_stdio()) {
        eprintln!("OpenDesk Google MCP bridge failed: {error:#}");
        std::process::exit(1);
    }
}
