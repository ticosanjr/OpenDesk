use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Default)]
pub struct FileQuery {
    pub workspace_id: Option<String>,
    pub path: Option<String>,
    pub range: Option<String>,
    pub q: Option<String>,
    pub limit: Option<usize>,
}

pub(super) async fn workspace_root(
    state: &AppState,
    workspace_id: Option<&str>,
) -> AppResult<PathBuf> {
    let workspace_id = crate::api::tasks::resolve_workspace_id(state, workspace_id).await?;
    let workspace = crate::db::workspaces::get(&state.db, &workspace_id).await?;
    std::fs::canonicalize(&workspace.root_path).map_err(|error| {
        AppError::BadRequest(format!(
            "workspace root is not accessible: {} ({error})",
            workspace.root_path
        ))
    })
}

pub(super) fn resolve_path(root: &Path, relative: Option<&str>) -> AppResult<PathBuf> {
    let relative = relative.unwrap_or("").trim();
    let candidate = if relative.is_empty() {
        root.to_path_buf()
    } else {
        if relative.starts_with('/') || relative.contains('\\') {
            return Err(AppError::BadRequest(
                "path must be relative to the workspace".into(),
            ));
        }
        root.join(relative)
    };
    let canonical = std::fs::canonicalize(&candidate)
        .map_err(|_| AppError::NotFound(format!("file {relative}")))?;
    if !canonical.starts_with(root) {
        return Err(AppError::BadRequest(
            "path escapes the workspace root".into(),
        ));
    }
    Ok(canonical)
}

pub(super) fn relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn should_skip(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | ".cache" | ".vite"
    )
}
