//! Startup, logging and shutdown helpers shared by the desktop shell and the
//! headless daemon entry points. `run()` itself stays in `lib.rs` so the Tauri
//! mobile entry point keeps its crate-root path.

use crate::app_state::AppState;
use crate::daemon_probe::ProbeOutcome;
use crate::paths::AppPaths;
use crate::server_lock::ServerLock;
use std::sync::{Arc, OnceLock};
#[cfg(feature = "desktop")]
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
#[cfg(feature = "desktop")]
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tracing_subscriber::EnvFilter;

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

/// Tauri does not translate Unix SIGTERM/SIGINT into RunEvent::Exit. Handle
/// them explicitly so the managed OpenCode child is not orphaned on logout,
/// IDE restarts or `kill`.
#[cfg(feature = "desktop")]
pub(crate) fn install_shutdown_handlers(app: AppHandle, state: Arc<AppState>) {
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
            crate::virtual_display::reap_all();
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
            crate::virtual_display::reap_all();
            app.exit(0);
        });
    }
}

/// Open (or focus) the floating Quick Capture window. This is the only window
/// besides the main one and it is intentionally tiny.
#[cfg(feature = "desktop")]
pub(crate) fn open_capture(app: &AppHandle) {
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

pub(crate) fn init_logging() {
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

#[cfg(feature = "desktop")]
/// Restart through the outer AppImage when applicable. Re-executing the inner
/// binary while the AppImage runtime unmounts can kill the new process.
fn relaunch(app: &tauri::AppHandle) {
    let executable = std::env::var_os("APPIMAGE")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_exe().ok());
    if let Some(executable) = executable {
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("setsid")
                .arg(&executable)
                .spawn();
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = std::process::Command::new(&executable).spawn();
        }
    }
    app.exit(0);
}

#[cfg(feature = "desktop")]
pub(crate) fn offer_v1_migration(app: tauri::AppHandle, paths: AppPaths, version: i64) {
    let out = paths.data_dir.join(format!(
        "v1-export-{}",
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    ));
    let message = format!(
        "Banco OpenDesk V1 detectado (schema {version}).\n\n\
         Os dados serão exportados para:\n{}\n\n\
         O arquivo antigo será preservado como opendesk.db.v1-<data> e o OpenDesk \
         iniciará com um banco V2 novo.",
        out.display()
    );
    let app_for_dialog = app.clone();
    app.dialog()
        .message(message)
        .title("OpenDesk — banco V1")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Exportar e continuar".into(),
            "Sair".into(),
        ))
        .show(move |accepted| {
            if !accepted {
                let app = app_for_dialog.clone();
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || handle.exit(0));
                return;
            }
            let app = app_for_dialog.clone();
            std::thread::spawn(move || {
                let result = tauri::async_runtime::block_on(opendesk_storage::legacy::migrate_v1(
                    &paths, &out,
                ));
                match result {
                    Ok(summary) => {
                        tracing::info!(
                            export = %summary.out.display(),
                            workspaces = summary.workspaces,
                            recipes = summary.recipes,
                            settings = summary.settings,
                            "V1 database exported; restarting with a fresh V2 database"
                        );
                        let app = app.clone();
                        let handle = app.clone();
                        let _ = app.run_on_main_thread(move || relaunch(&handle));
                    }
                    Err(error) => {
                        tracing::error!(error = %error, "V1 database migration failed");
                        let app = app.clone();
                        let handle = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            let exit_handle = handle.clone();
                            handle
                                .dialog()
                                .message(format!(
                                    "Falha ao exportar/migrar o banco V1:\n\n{error:#}"
                                ))
                                .title("OpenDesk")
                                .kind(MessageDialogKind::Error)
                                .buttons(MessageDialogButtons::Ok)
                                .show(move |_| {
                                    exit_handle.exit(1);
                                });
                        });
                    }
                }
            });
        });
}

/// Report a fatal startup error and leave with status 1. Headless entry points
/// already own the process, so an explicit exit is clearer than a panic.
fn fatal(message: String) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

/// Headless mode used by browser tests and future daemon-only installs.
/// It runs the same AppState, SQLite, watcher and Axum API without a Tauri
/// window or tray. `OPENDESK_SKIP_DAEMON=1` keeps the OpenCode engine offline.
pub fn run_headless() {
    init_logging();

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => fatal(format!("tokio runtime: {error}")),
    };
    runtime.block_on(async {
        let paths = match AppPaths::discover() {
            Ok(paths) => paths,
            Err(error) => fatal(format!("application paths: {error}")),
        };

        // Never start a second backend on the same data directory. A healthy
        // peer wins; an unhealthy peer leaves a stale lock that gets replaced.
        if let ProbeOutcome::Remote(endpoint) = crate::daemon_probe::probe_outcome(&paths).await {
            println!(
                "OpenDesk already running on http://{}:{}; not starting another daemon",
                endpoint.host, endpoint.port
            );
            return;
        }
        let lock = match ServerLock::acquire(&paths) {
            Ok(lock) => lock,
            Err(error) => fatal(format!("OpenDesk server lock: {error}")),
        };
        let Some(_lock) = lock else {
            eprintln!("OpenDesk: another process holds the local server lock; exiting");
            std::process::exit(1);
        };

        let state = Arc::new(match AppState::initialize(paths).await {
            Ok(state) => state,
            Err(error) => fatal(format!("initialize OpenDesk state: {error}")),
        });

        if let Err(error) = crate::control_plane::start(
            state.clone(),
            crate::control_plane::StartOptions {
                daemon: std::env::var("OPENDESK_SKIP_DAEMON").is_err(),
                integrations: true,
            },
        )
        .await
        {
            fatal(format!("control plane startup: {error}"));
        }
        if let Err(error) = crate::api::spawn_server(state.clone()).await {
            fatal(format!("OpenDesk API server: {error}"));
        }

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
        crate::virtual_display::reap_all();
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
