use crate::{app_state::AppState, paths::AppPaths, shell_state::ShellState};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;

/// Opens a path only when it belongs to a registered workspace or to one of the
/// app-owned directories. The JavaScript opener plugin has a static ACL scope,
/// which cannot know the user's configurable workspace roots.
#[tauri::command]
pub async fn open_workspace_path(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let shell = app.state::<Arc<ShellState>>().inner().clone();
    let candidate =
        std::fs::canonicalize(&path).map_err(|error| format!("caminho inválido: {error}"))?;
    let (roots, paths) = match shell.as_ref() {
        ShellState::Embedded(state) => (embedded_roots(state).await?, state.paths.clone()),
        ShellState::Remote(remote) => (
            remote
                .workspace_roots()
                .await
                .map_err(|error| format!("workspaces: {error}"))?,
            remote.paths.clone(),
        ),
    };
    if !path_allowed(&candidate, &roots, &paths) {
        return Err("caminho fora dos workspaces e diretórios do OpenDesk".into());
    }
    app.opener()
        .open_path(candidate.to_string_lossy().to_string(), None::<&str>)
        .map_err(|error| format!("não foi possível abrir: {error}"))
}

async fn embedded_roots(state: &AppState) -> Result<Vec<String>, String> {
    Ok(crate::db::workspaces::list(&state.db)
        .await
        .map_err(|error| format!("workspaces: {error}"))?
        .into_iter()
        .map(|workspace| workspace.root_path)
        .collect())
}

fn path_allowed(candidate: &Path, roots: &[String], paths: &AppPaths) -> bool {
    for root in roots {
        if let Ok(root) = std::fs::canonicalize(root)
            && candidate.starts_with(root)
        {
            return true;
        }
    }
    [
        &paths.data_dir,
        &paths.config_dir,
        &paths.cache_dir,
        &paths.log_dir,
        &paths.backups_dir,
        &paths.artifacts_dir,
        &paths.run_logs_dir,
    ]
    .iter()
    .any(|directory| candidate.starts_with(canonical_or_original(directory)))
}

fn canonical_or_original(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_workspace_roots_and_app_directories_only() {
        let dir = tempfile::tempdir().expect("temp");
        let paths = crate::test_support::paths_in(dir.path());
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(workspace.join("output")).expect("workspace");
        let inside = std::fs::canonicalize(workspace.join("output")).expect("inside");
        let outside = std::fs::canonicalize(dir.path().join("..")).expect("outside");

        let roots = vec![workspace.to_string_lossy().to_string()];
        assert!(path_allowed(&inside, &roots, &paths));
        assert!(!path_allowed(&outside, &roots, &paths));

        let logs = std::fs::canonicalize(&paths.log_dir).expect("logs");
        assert!(path_allowed(&logs, &[], &paths));
    }
}
