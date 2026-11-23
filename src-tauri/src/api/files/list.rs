use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Serialize;
use std::sync::Arc;

use super::paths::{FileQuery, relative_to, resolve_path, should_skip, workspace_root};

const MAX_ENTRIES: usize = 400;

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<String>,
}

pub async fn tree(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileQuery>,
) -> AppResult<Json<Vec<FileEntry>>> {
    let root = workspace_root(&state, query.workspace_id.as_deref()).await?;
    let directory = resolve_path(&root, query.path.as_deref())?;
    let metadata = std::fs::metadata(&directory)?;
    if !metadata.is_dir() {
        return Err(AppError::BadRequest("path is not a directory".into()));
    }

    let mut entries = Vec::new();
    for entry in std::fs::read_dir(&directory)? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if should_skip(&name) {
            continue;
        }
        let path = entry.path();
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        entries.push(FileEntry {
            name,
            path: relative_to(&root, &path),
            is_dir: metadata.is_dir(),
            size: metadata.len(),
            modified: metadata
                .modified()
                .ok()
                .map(|value| chrono::DateTime::<chrono::Utc>::from(value).to_rfc3339()),
        });
        if entries.len() >= MAX_ENTRIES {
            break;
        }
    }

    entries.sort_by(|left, right| {
        right
            .is_dir
            .cmp(&left.is_dir)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(Json(entries))
}
