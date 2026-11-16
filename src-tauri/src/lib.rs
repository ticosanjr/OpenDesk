mod api;
mod app_state;
mod audit;
mod backup;
mod browse;
mod chat;
mod cli;
mod computer;
mod config_sync;
mod control_plane;
mod daemon_probe;
mod desktop_integration;
mod doctor;
mod dsh_router;
mod governance;
mod hitl;
mod integrations;
mod internal_api;
mod io_bus;
mod learning;
mod mcp_bridge;
mod mcp_config;
mod mcp_control;
mod mcp_hub;
mod mcp_server;
mod media;
mod metrics;
mod open_paths;
mod personal_context;
mod proposals;
mod push;
mod queue_scheduler;
mod recipes;
mod run_export;
mod run_notifier;
mod run_timeline;
#[allow(dead_code)]
mod session_context;
mod shell_state;
mod skill_eval;
mod skill_gate;
mod starter_packs;
#[cfg(test)]
mod test_support;
mod tray;
mod update_signing;
mod updates;
mod voice;

pub use opendesk_core::{
    adapters, db, dedupe, error, events, nodes, notifier, opencode_client, passkey, paths, runtime,
    sandbox, secrets, server_lock, text, token_estimate, virtual_display, workspace_sync, worktree,
};

use app_state::AppState;
use daemon_probe::ProbeOutcome;
use paths::AppPaths;
use server_lock::ServerLock;
use shell_state::{RemoteShell, ShellState};
use std::sync::{Arc, OnceLock};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::ShortcutState;
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
            state.io_bus.stop().await;
            state.write_buffer.shutdown().await;
            state.queue.shutdown().await;
            state.signal.stop().await;
            state.telegram.stop().await;
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
            state.io_bus.stop().await;
            state.write_buffer.shutdown().await;
            state.queue.shutdown().await;
            state.signal.stop().await;
            state.telegram.stop().await;
            state.daemon.shutdown().await;
            app.exit(0);
        });
    }
}

/// Open (or focus) the floating Quick Capture window. This is the only window
/// besides the main one and it is intentionally tiny.
fn open_capture(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("capture") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(
        app,
        "capture",
        WebviewUrl::App("index.html#/capture".into()),
    )
    .title("Quick Capture")
    .inner_size(620.0, 360.0)
    .min_inner_size(480.0, 240.0)
    .resizable(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .decorations(false)
    .visible(false)
    .build()
    .map(|window| {
        let _ = window.show();
        let _ = window.set_focus();
    });
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
        // Single-instance must initialize before global shortcuts. Otherwise a
        // second launch fails while registering the hotkey and never reaches
        // the single-instance guard.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        open_capture(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init());

    let app = builder
        .setup(|app| {
            let paths = AppPaths::discover()?;
            if let Err(error) = desktop_integration::ensure(&paths, env!("CARGO_PKG_VERSION")) {
                tracing::warn!(error = %error, "desktop shortcut integration failed");
            }

            // Attach to a healthy daemon before creating any local state. This
            // keeps `api.json`, the database and the API port owned by exactly
            // one process even when the desktop starts after `opendeskd`.
            let shell = Arc::new(
                match tauri::async_runtime::block_on(daemon_probe::probe_outcome(&paths)) {
                    ProbeOutcome::Remote(endpoint) => {
                        tracing::info!(
                            host = %endpoint.host,
                            port = endpoint.port,
                            "attaching to an external OpenDesk daemon"
                        );
                        ShellState::Remote(Arc::new(RemoteShell::new(endpoint, paths)))
                    }
                    ProbeOutcome::Embedded => {
                        let Some(lock) = ServerLock::acquire(&paths)? else {
                            let pid = ServerLock::pid(&paths)
                                .map(|pid| pid.to_string())
                                .unwrap_or_else(|| "desconhecido".into());
                            return Err(anyhow::anyhow!(
                                "outro processo OpenDesk (pid {pid}) já controla a API local ({})",
                                paths.api_lock_path.display()
                            )
                            .into());
                        };
                        app.manage(lock);

                        let state =
                            Arc::new(tauri::async_runtime::block_on(AppState::initialize(paths))?);
                        state.cron.attach_state(&state);
                        tauri::async_runtime::block_on(control_plane::start(
                            state.clone(),
                            control_plane::StartOptions::default(),
                        ))?;
                        tauri::async_runtime::block_on(api::spawn_server(state.clone()))?;
                        install_shutdown_handlers(app.handle().clone(), state.clone());
                        ShellState::Embedded(state)
                    }
                },
            );

            app.manage(shell.clone());
            tracing::info!(mode = shell.mode(), "desktop shell ready");
            use tauri_plugin_global_shortcut::GlobalShortcutExt;
            if let Err(error) = app
                .global_shortcut()
                .register("CommandOrControl+Shift+Space")
            {
                tracing::warn!(error = %error, "Quick Capture global shortcut unavailable");
            }
            tray::setup(app.handle(), shell)?;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.maximize();
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Focused(false) = event
                && window.label() == "capture"
            {
                let _ = window.hide();
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Only the main window belongs to the tray lifecycle. The
                // Quick Capture window must close normally.
                if window.label() != "main" {
                    return;
                }
                let app = window.app_handle();
                let close_to_tray = app
                    .try_state::<Arc<ShellState>>()
                    .map(|shell| match shell.inner().as_ref() {
                        ShellState::Embedded(state) => tauri::async_runtime::block_on(async {
                            crate::db::settings::get(&state.db, "close_to_tray")
                                .await
                                .ok()
                                .flatten()
                                .and_then(|value| value.as_bool())
                                .unwrap_or(true)
                        }),
                        // Remote mode owns no local state; closing to the tray
                        // keeps the daemon and its runs alive independently.
                        ShellState::Remote(_) => true,
                    })
                    .unwrap_or(true);
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    app.exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            api::commands::get_api_endpoint,
            api::commands::get_daemon_status,
            open_paths::open_workspace_path
        ])
        .build(tauri::generate_context!())
        .expect("error while building the OpenDesk application");

    app.run(|app_handle, event| {
        if let tauri::RunEvent::Exit = event
            && let Some(shell) = app_handle.try_state::<Arc<ShellState>>()
            && let Some(state) = shell.inner().embedded().cloned()
        {
            tauri::async_runtime::block_on(async {
                state.cron.shutdown();
                state.write_buffer.shutdown().await;
                state.queue.shutdown().await;
                state.daemon.shutdown().await;
            });
        }
    });
}

/// Headless mode used by browser tests and future daemon-only installs.
/// It runs the same AppState, SQLite, watcher and Axum API without a Tauri
/// window or tray. `OPENDESK_SKIP_DAEMON=1` keeps the OpenCode engine offline.
pub fn run_cli(args: Vec<String>) {
    cli::run(args);
}

/// `opendeskd` entry point. Kept as a thin alias so future service packaging
/// can add daemon-specific configuration without changing the desktop binary.
pub fn run_daemon() {
    run_headless();
}

pub fn run_headless() {
    init_logging();

    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    runtime.block_on(async {
        let paths = AppPaths::discover().expect("application paths");

        // Never start a second backend on the same data directory. A healthy
        // peer wins; an unhealthy peer leaves a stale lock that gets replaced.
        if let ProbeOutcome::Remote(endpoint) = daemon_probe::probe_outcome(&paths).await {
            println!(
                "OpenDesk already running on http://{}:{}; not starting another daemon",
                endpoint.host, endpoint.port
            );
            return;
        }
        let Some(_lock) = ServerLock::acquire(&paths).expect("OpenDesk server lock") else {
            eprintln!("OpenDesk: another process holds the local server lock; exiting");
            std::process::exit(1);
        };

        let state = Arc::new(
            AppState::initialize(paths)
                .await
                .expect("initialize OpenDesk state"),
        );

        control_plane::start(
            state.clone(),
            control_plane::StartOptions {
                daemon: std::env::var("OPENDESK_SKIP_DAEMON").is_err(),
                integrations: true,
            },
        )
        .await
        .expect("control plane startup");
        api::spawn_server(state.clone())
            .await
            .expect("OpenDesk API server");

        println!(
            "OpenDesk headless API on http://{}:{} (token in api.json)",
            state.api_host, state.api_port
        );

        wait_for_daemon_shutdown().await;
        state.cron.shutdown();
        state.io_bus.stop().await;
        state.write_buffer.shutdown().await;
        state.queue.shutdown().await;
        state.daemon.shutdown().await;
    });
}

async fn wait_for_daemon_shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut terminate = match signal(SignalKind::terminate()) {
            Ok(signal) => signal,
            Err(error) => {
                tracing::warn!(error = %error, "could not install SIGTERM handler");
                let _ = tokio::signal::ctrl_c().await;
                return;
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = terminate.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// `opendesk doctor` — preflight for the engine and local state without a GUI.
pub fn run_doctor(json: bool, fix_daemons: bool, live: bool) {
    std::process::exit(doctor::run(json, fix_daemons, live));
}

/// Local update-signing utilities (`keygen-update`, `sign-update`). They are
/// used by `scripts/release.sh`, never by the running app.
pub fn run_update_signing(args: Vec<String>) {
    if let Err(error) = update_signing::run_cli(&args) {
        eprintln!("opendesk update signing: {error:#}");
        std::process::exit(1);
    }
}

/// MCP server over stdio used by OpenCode's `mcp.servers.opendesk-google`.
/// Stdout is reserved for JSON-RPC frames; diagnostics go to stderr.
pub fn run_mcp_config() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    if let Err(error) = runtime.block_on(mcp_config::run_stdio()) {
        eprintln!("OpenDesk config MCP bridge failed: {error:#}");
        std::process::exit(1);
    }
}

/// MCP server over stdio used by OpenCode's `mcp.servers.opendesk-control`.
/// It talks to the local API so queueing, validation, events and audit stay in
/// the backend. Stdout is reserved for JSON-RPC frames.
pub fn run_mcp_control() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    if let Err(error) = runtime.block_on(mcp_control::run_stdio()) {
        eprintln!("OpenDesk control MCP bridge failed: {error:#}");
        std::process::exit(1);
    }
}

pub fn run_mcp_google() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    if let Err(error) = runtime.block_on(mcp_bridge::run_stdio()) {
        eprintln!("OpenDesk Google MCP bridge failed: {error:#}");
        std::process::exit(1);
    }
}
