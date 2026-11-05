mod api;
mod app_state;
mod db;
mod error;
mod events;
mod opencode_client;
mod paths;
mod tray;

use app_state::AppState;
use paths::AppPaths;
use std::sync::Arc;
use tauri::Manager;
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("opendesk=debug,opencode=info,tower_http=info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init();

    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
            tray::setup(app.handle(), state.clone())?;
            tauri::async_runtime::block_on(api::spawn_server(state.clone()))?;

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
        if let tauri::RunEvent::Exit = event {
            if let Some(state) = app_handle.try_state::<Arc<AppState>>() {
                let state = state.inner().clone();
                tauri::async_runtime::block_on(state.daemon.shutdown());
            }
        }
    });
}
