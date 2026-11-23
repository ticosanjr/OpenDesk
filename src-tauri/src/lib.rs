mod api;
mod app_state;
mod bootstrap;
mod chat;
mod cli;
mod computer;
mod control_plane;
mod doctor;
mod governance;
mod integrations;
mod io_bus;
mod learning;
mod mcp;
#[cfg(feature = "desktop")]
mod open_paths;
mod proposals;
mod queue_scheduler;
mod recipe_dispatcher;
mod recipes;
mod run_notifier;
#[allow(dead_code)]
mod session_context;
mod shell_queue_ports;
#[cfg(feature = "desktop")]
mod shell_state;
mod skill_gate;
mod skill_shadow_exec;
mod starter_packs;
#[cfg(test)]
#[path = "test_db_support.rs"]
mod test_db_support;
#[cfg(test)]
#[path = "test_support_tests.rs"]
mod test_support;
#[cfg(feature = "desktop")]
mod tray;
mod updates;
mod v1_export;
mod voice;

pub mod audit;
pub mod backup;
pub mod db;
pub mod desktop_integration;
pub mod error;
pub mod internal_api;
pub mod metrics;
pub mod passkey;

pub use mcp::{
    bridge as mcp_bridge, control as mcp_control, hub as mcp_hub, sampling as mcp_sampling,
    session as mcp_session,
};
pub use opendesk_channels::webpush as push;
pub use opendesk_domain::hitl;
pub use opendesk_engine::scheduler::recurrence;
pub use opendesk_engine::{
    adapters, api_endpoint, browse, browse_tags, config_sync, daemon_probe, dsh_router, media,
    notifier, opencode_client, personal_context, queue_ports, recipe_dispatch, run_timeline,
    runtime, sandbox, text, token_estimate, worktree,
};
pub use opendesk_enterprise::{
    auth::mtls,
    auth::update_signing,
    cluster::{nodes, sync as workspace_sync},
    computer::virtual_display,
    eval::{cases as skill_cases, shadow as skill_shadow, skills as skill_eval},
};
pub use opendesk_mcp::{
    config as mcp_config, pool as mcp_pool, rate as mcp_rate, rbac as mcp_rbac,
    server as mcp_server,
};
pub use opendesk_recipes::context as recipe_context;
pub use opendesk_recipes::export as run_export;
pub use opendesk_storage::{dedupe, events, paths, secrets, server_lock};

use app_state::AppState;
use daemon_probe::ProbeOutcome;
use paths::AppPaths;
use server_lock::ServerLock;
#[cfg(feature = "desktop")]
use shell_state::{RemoteShell, ShellState};
use std::sync::Arc;
#[cfg(feature = "desktop")]
use tauri::Manager;
#[cfg(feature = "desktop")]
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
#[cfg(feature = "desktop")]
use tauri_plugin_global_shortcut::ShortcutState;

#[cfg(feature = "desktop")]
use bootstrap::{init_logging, install_shutdown_handlers, offer_v1_migration, open_capture};

#[cfg(feature = "desktop")]
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
            let result: Result<(), Box<dyn std::error::Error>> = (|| {
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
                            // The V2 schema refuses a V1 database. Offer the export
                            // before acquiring locks, showing the window or the tray.
                            if let Some(version) = tauri::async_runtime::block_on(
                                opendesk_storage::legacy::legacy_version(&paths.db_path),
                            )? {
                                offer_v1_migration(app.handle().clone(), paths, version);
                                return Ok(());
                            }
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
                    let _ = window.show();
                    let _ = window.maximize();
                }
                Ok(())
            })();

            if let Err(error) = result {
                tracing::error!(error = %error, "OpenDesk startup failed");
                let handle = app.handle().clone();
                app.dialog()
                    .message(format!("Falha ao iniciar o OpenDesk:\n\n{error}"))
                    .title("OpenDesk")
                    .kind(MessageDialogKind::Error)
                    .buttons(MessageDialogButtons::Ok)
                    .show(move |_| {
                        handle.exit(1);
                    });
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
                crate::virtual_display::reap_all();
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

/// `opendesk migrate-v1-export`: reachable from the AppImage, which does not
/// ship the separate `opendesk-cli` binary.
pub fn run_v1_export(args: Vec<String>) {
    v1_export::run(args);
}

/// `opendeskd` entry point. Kept as a thin alias so future service packaging
/// can add daemon-specific configuration without changing the desktop binary.
pub fn run_daemon() {
    run_headless();
}

/// Headless entry point shared with the browser tests and daemon-only installs.
pub use bootstrap::run_headless;

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
