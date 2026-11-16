use crate::{
    app_state::ApiEndpoint, opencode_client::daemon_manager::DaemonStatus, shell_state::ShellState,
};
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn get_api_endpoint(state: State<'_, Arc<ShellState>>) -> ApiEndpoint {
    state.api_endpoint()
}

#[tauri::command]
pub async fn get_daemon_status(state: State<'_, Arc<ShellState>>) -> Result<DaemonStatus, String> {
    match state.inner().as_ref() {
        ShellState::Embedded(state) => Ok(state.daemon.status().await),
        ShellState::Remote(remote) => {
            match remote.get_json::<DaemonStatus>("/api/daemon/status").await {
                Ok(status) => Ok(status),
                Err(error) => Ok(DaemonStatus {
                    online: false,
                    state: "remote-unavailable".into(),
                    message: Some(error.to_string()),
                    mode: "remote".into(),
                    ..DaemonStatus::default()
                }),
            }
        }
    }
}
