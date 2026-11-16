use crate::{
    app_state::AppState,
    events::{OpenDeskWsEvent, WsEnvelope},
    opencode_client::daemon_manager::DaemonStatus,
    queue_scheduler::QueueStatus,
    shell_state::ShellState,
};
use reqwest::Method;
use std::{sync::Arc, time::Duration};
use tauri::{
    AppHandle, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
};

pub fn setup(app: &AppHandle, shell: Arc<ShellState>) -> tauri::Result<()> {
    let version = app.package_info().version.to_string();
    let version_item = MenuItem::with_id(
        app,
        "app_version",
        version_label(&version),
        false,
        None::<&str>,
    )?;
    let status_item = MenuItem::with_id(
        app,
        "daemon_status",
        initial_status(&shell),
        false,
        None::<&str>,
    )?;
    let queue_item = MenuItem::with_id(
        app,
        "queue_status",
        "Fila: 0 rodando | 0 aguardando",
        false,
        None::<&str>,
    )?;
    let spend_item = MenuItem::with_id(
        app,
        "spend_status",
        "Gasto hoje: $0.00 / $5.00",
        false,
        None::<&str>,
    )?;
    let pause_item = MenuItem::with_id(app, "pause_queue", "Pausar fila", true, None::<&str>)?;
    let open_item =
        MenuItem::with_id(app, "open_dashboard", "Abrir Dashboard", true, None::<&str>)?;
    let restart_item = MenuItem::with_id(
        app,
        "restart_daemon",
        "Reiniciar OpenCode Daemon",
        true,
        None::<&str>,
    )?;
    let quit_item = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let separator_zero = PredefinedMenuItem::separator(app)?;
    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[
            &version_item,
            &separator_zero,
            &status_item,
            &queue_item,
            &spend_item,
            &separator_one,
            &pause_item,
            &open_item,
            &restart_item,
            &separator_two,
            &quit_item,
        ],
    )?;

    // Use the same mark shown next to "OpenDesk" in the app. The generated
    // 32px icon is the tray-optimized version of public/opendesk.svg.
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))?;

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip(version_label(&version))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Up,
                ..
            } = event
            {
                focus_main(tray.app_handle());
            }
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "quit" => app.exit(0),
            "open_dashboard" => focus_main(app),
            "restart_daemon" => {
                let shell = app.state::<Arc<ShellState>>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    match shell.as_ref() {
                        ShellState::Embedded(state) => state.daemon.restart().await,
                        ShellState::Remote(remote) => {
                            let _ = remote.call(Method::POST, "/api/daemon/restart", None).await;
                        }
                    }
                });
            }
            "pause_queue" => {
                let shell = app.state::<Arc<ShellState>>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    match shell.as_ref() {
                        ShellState::Embedded(state) => {
                            if state.queue.is_paused() {
                                let _ = state.queue.resume().await;
                            } else {
                                state.queue.pause("paused from tray").await;
                            }
                        }
                        ShellState::Remote(remote) => {
                            let paused = remote
                                .get_json::<QueueStatus>("/api/queue/status")
                                .await
                                .map(|status| status.paused)
                                .unwrap_or(false);
                            let path = if paused {
                                "/api/queue/resume"
                            } else {
                                "/api/queue/pause?reason=paused%20from%20tray"
                            };
                            let _ = remote.call(Method::POST, path, None).await;
                        }
                    }
                });
            }
            _ => {}
        })
        .build(app)?;

    spawn_menu_updater(
        app.clone(),
        shell,
        status_item,
        queue_item,
        pause_item,
        spend_item,
    );
    Ok(())
}

fn initial_status(shell: &ShellState) -> String {
    match shell {
        ShellState::Embedded(_) => "OpenCode Engine: iniciando…".into(),
        ShellState::Remote(remote) => {
            format!("OpenDesk daemon externo (:{})", remote.endpoint.port)
        }
    }
}

fn spawn_menu_updater(
    app: AppHandle,
    shell: Arc<ShellState>,
    status_item: MenuItem<tauri::Wry>,
    queue_item: MenuItem<tauri::Wry>,
    pause_item: MenuItem<tauri::Wry>,
    _spend_item: MenuItem<tauri::Wry>,
) {
    match shell.as_ref() {
        ShellState::Embedded(state) => {
            spawn_embedded_updater(app, state.clone(), status_item, queue_item, pause_item);
        }
        ShellState::Remote(remote) => {
            let remote = remote.clone();
            let _ = app;
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(5));
                loop {
                    interval.tick().await;

                    if !remote.health().await {
                        let _ = status_item.set_text("OpenDesk daemon externo indisponível");
                        let _ = queue_item.set_text("Fila: daemon externo indisponível");
                        continue;
                    }

                    match remote.get_json::<QueueStatus>("/api/queue/status").await {
                        Ok(status) => {
                            let _ = queue_item.set_text(format!(
                                "Fila: {} rodando | {} aguardando",
                                status.running, status.waiting
                            ));
                            let _ = pause_item.set_text(if status.paused {
                                "Retomar fila"
                            } else {
                                "Pausar fila"
                            });
                        }
                        Err(_) => {
                            let _ = queue_item.set_text("Fila: daemon externo indisponível");
                        }
                    }

                    match remote.get_json::<DaemonStatus>("/api/daemon/status").await {
                        Ok(status) => {
                            let label = if status.online {
                                format!("OpenCode Engine (:{}) [Online]", status.port)
                            } else {
                                format!("OpenCode Engine [{}]", status.state)
                            };
                            let _ = status_item.set_text(label);
                        }
                        Err(_) => {
                            let _ = status_item.set_text("OpenDesk daemon externo indisponível");
                        }
                    }
                }
            });
        }
    }
}

fn spawn_embedded_updater(
    app: AppHandle,
    state: Arc<AppState>,
    status_item: MenuItem<tauri::Wry>,
    queue_item: MenuItem<tauri::Wry>,
    pause_item: MenuItem<tauri::Wry>,
) {
    tauri::async_runtime::spawn(async move {
        let mut receiver = state.events.subscribe();
        while let Ok(WsEnvelope { event, .. }) = receiver.recv().await {
            match event {
                OpenDeskWsEvent::DaemonStatusChanged {
                    online,
                    port,
                    state,
                    ..
                } => {
                    let label = if online {
                        format!("OpenCode Engine (:{port}) [Online]")
                    } else {
                        format!("OpenCode Engine [{state}]")
                    };
                    let _ = status_item.set_text(label);
                }
                OpenDeskWsEvent::QueueUpdated { jobs, .. } => {
                    let running = jobs.iter().filter(|job| job.status == "RUNNING").count();
                    let waiting = jobs
                        .iter()
                        .filter(|job| job.status == "QUEUED" || job.status == "PAUSED")
                        .count();
                    let _ = queue_item
                        .set_text(format!("Fila: {running} rodando | {waiting} aguardando"));
                    let _ = pause_item.set_text(if state.queue.is_paused() {
                        "Retomar fila"
                    } else {
                        "Pausar fila"
                    });
                }
                _ => {}
            }
        }

        // Keep the handle alive for the duration of the updater.
        let _ = app;
    });
}

fn version_label(version: &str) -> String {
    format!("OpenDesk v{version}")
}

fn focus_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::version_label;

    #[test]
    fn tray_version_label_is_explicit() {
        assert_eq!(version_label("0.15.1"), "OpenDesk v0.15.1");
    }
}
