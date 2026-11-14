use crate::app_state::AppState;
use std::sync::Arc;
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

/// Opens a path only when it belongs to a registered workspace or to one of the
/// app-owned directories. The JavaScript opener plugin has a static ACL scope,
/// which cannot know the user's configurable workspace roots.
#[tauri::command]
pub async fn open_workspace_path(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let state = app.state::<Arc<AppState>>().inner().clone();
    let candidate =
        std::fs::canonicalize(&path).map_err(|error| format!("caminho inválido: {error}"))?;
    let mut allowed = false;
    for workspace in crate::db::workspaces::list(&state.db)
        .await
        .map_err(|error| format!("workspaces: {error}"))?
    {
        let Ok(root) = std::fs::canonicalize(&workspace.root_path) else {
            continue;
        };
        if candidate.starts_with(&root) {
            allowed = true;
            break;
        }
    }
    if !allowed {
        for directory in [
            &state.paths.data_dir,
            &state.paths.config_dir,
            &state.paths.cache_dir,
            &state.paths.log_dir,
            &state.paths.backups_dir,
            &state.paths.artifacts_dir,
            &state.paths.run_logs_dir,
        ] {
            if candidate.starts_with(directory) {
                allowed = true;
                break;
            }
        }
    }
    if !allowed {
        return Err("caminho fora dos workspaces e diretórios do OpenDesk".into());
    }
    app.opener()
        .open_path(candidate.to_string_lossy().to_string(), None::<&str>)
        .map_err(|error| format!("não foi possível abrir: {error}"))
}
