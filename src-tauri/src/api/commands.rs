use crate::{
    app_state::{ApiEndpoint, AppState},
    opencode_client::daemon_manager::DaemonStatus,
};
use tauri::State;

#[tauri::command]
pub fn get_api_endpoint(state: State<'_, std::sync::Arc<AppState>>) -> ApiEndpoint {
    state.api_endpoint()
}

#[tauri::command]
pub async fn get_daemon_status(
    state: State<'_, std::sync::Arc<AppState>>,
) -> Result<DaemonStatus, String> {
    let state = state.inner().clone();
    Ok(state.daemon.status().await)
}
