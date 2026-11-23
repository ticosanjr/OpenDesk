use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Serialize;
use std::{path::Path, sync::Arc};

use super::paths::{FileQuery, resolve_path, workspace_root};

#[derive(Debug, Clone, Serialize)]
pub struct ArchiveEntry {
    pub name: String,
    pub size: u64,
    pub compressed: u64,
    pub is_dir: bool,
}

const MAX_ARCHIVE_ENTRIES: usize = 400;

/// List the entries of a ZIP archive without extracting anything. Entries are
/// capped so a huge package never stalls the viewer.
pub fn archive_entries(path: &Path) -> AppResult<Vec<ArchiveEntry>> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| AppError::BadRequest(format!("não é um arquivo ZIP: {error}")))?;
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        if entries.len() >= MAX_ARCHIVE_ENTRIES {
            break;
        }
        let entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(error) => {
                // A single broken member must not hide the rest of the archive.
                tracing::warn!(index, error = %error, "skipping unreadable ZIP entry");
                continue;
            }
        };
        entries.push(ArchiveEntry {
            name: String::from_utf8_lossy(entry.name_raw()).to_string(),
            size: entry.size(),
            compressed: entry.compressed_size(),
            is_dir: entry.is_dir(),
        });
    }
    entries.sort_by(|left, right| {
        right
            .is_dir
            .cmp(&left.is_dir)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(entries)
}

pub async fn archive(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileQuery>,
) -> AppResult<Json<Vec<ArchiveEntry>>> {
    let root = workspace_root(&state, query.workspace_id.as_deref()).await?;
    let path = resolve_path(&root, query.path.as_deref())?;
    let metadata = std::fs::metadata(&path)?;
    if !metadata.is_file() {
        return Err(AppError::BadRequest("path is not a file".into()));
    }
    Ok(Json(archive_entries(&path)?))
}
