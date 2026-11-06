use crate::{
    app_state::AppState,
    events::{OpenDeskWsEvent, WsEnvelope},
};
use std::sync::Arc;
use tauri::{
    AppHandle, Manager,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};

pub fn setup(app: &AppHandle, state: Arc<AppState>) -> tauri::Result<()> {
    let status_item = MenuItem::with_id(
        app,
        "daemon_status",
        "OpenCode Engine: iniciando…",
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
    let separator_one = PredefinedMenuItem::separator(app)?;
    let separator_two = PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[
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

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?;

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("OpenDesk")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "quit" => app.exit(0),
            "open_dashboard" => focus_main(app),
            "restart_daemon" => {
                let state = app.state::<Arc<AppState>>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    state.daemon.restart().await;
                });
            }
            "pause_queue" => {
                let state = app.state::<Arc<AppState>>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    if state.queue.is_paused() {
                        let _ = state.queue.resume().await;
                    } else {
                        state.queue.pause("paused from tray").await;
                    }
                });
            }
            _ => {}
        })
        .build(app)?;

    spawn_menu_updater(
        app.clone(),
        state,
        status_item,
        queue_item,
        pause_item,
        spend_item,
    );
    Ok(())
}

fn spawn_menu_updater(
    app: AppHandle,
    state: Arc<AppState>,
    status_item: MenuItem<tauri::Wry>,
    queue_item: MenuItem<tauri::Wry>,
    pause_item: MenuItem<tauri::Wry>,
    _spend_item: MenuItem<tauri::Wry>,
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

fn focus_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
