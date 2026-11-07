use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};
use axum::{
    Json,
    body::Body,
    extract::{Query, State},
    http::{StatusCode, header},
    response::Response,
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

const MAX_PREVIEW_BYTES: usize = 512 * 1024;
const MAX_ENTRIES: usize = 400;

#[derive(Debug, Deserialize, Default)]
pub struct FileQuery {
    pub workspace_id: Option<String>,
    pub path: Option<String>,
    pub range: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FileContent {
    pub path: String,
    pub content: String,
    pub truncated: bool,
    pub binary: bool,
    pub mime: String,
    pub size: u64,
}

async fn workspace_root(state: &AppState, workspace_id: Option<&str>) -> AppResult<PathBuf> {
    let workspace_id = crate::api::tasks::resolve_workspace_id(state, workspace_id).await?;
    let workspace = crate::db::workspaces::get(&state.db, &workspace_id).await?;
    std::fs::canonicalize(&workspace.root_path).map_err(|error| {
        AppError::BadRequest(format!(
            "workspace root is not accessible: {} ({error})",
            workspace.root_path
        ))
    })
}

fn resolve_path(root: &Path, relative: Option<&str>) -> AppResult<PathBuf> {
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

fn relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn should_skip(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | ".cache" | ".vite"
    )
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

pub async fn content(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileQuery>,
) -> AppResult<Json<FileContent>> {
    let root = workspace_root(&state, query.workspace_id.as_deref()).await?;
    let path = resolve_path(&root, query.path.as_deref())?;
    let metadata = std::fs::metadata(&path)?;
    if !metadata.is_file() {
        return Err(AppError::BadRequest("path is not a file".into()));
    }
    let bytes = std::fs::read(&path)?;
    let truncated = bytes.len() > MAX_PREVIEW_BYTES;
    let slice = &bytes[..bytes.len().min(MAX_PREVIEW_BYTES)];
    let binary =
        slice.iter().take(8_000).any(|byte| *byte == 0) || std::str::from_utf8(slice).is_err();
    let content = if binary {
        String::new()
    } else {
        String::from_utf8_lossy(slice).to_string()
    };
    Ok(Json(FileContent {
        path: relative_to(&root, &path),
        content,
        truncated,
        binary,
        mime: mime_for(&path).to_string(),
        size: metadata.len(),
    }))
}

pub async fn raw(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileQuery>,
) -> AppResult<Response> {
    let root = workspace_root(&state, query.workspace_id.as_deref()).await?;
    let path = resolve_path(&root, query.path.as_deref())?;
    let metadata = std::fs::metadata(&path)?;
    if !metadata.is_file() {
        return Err(AppError::BadRequest("path is not a file".into()));
    }
    let (bytes, total, range) = read_raw(&path, query.range.as_deref())?;
    let filename = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());
    let mut builder = Response::builder()
        .header(header::CONTENT_TYPE, mime_for(&path))
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{filename}\""),
        )
        .header(header::CONTENT_LENGTH, bytes.len())
        .header(header::ACCEPT_RANGES, "bytes");
    if let Some((start, end)) = range {
        builder = builder.status(StatusCode::PARTIAL_CONTENT).header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total}"),
        );
    }
    builder
        .body(Body::from(bytes))
        .map_err(|error| AppError::Internal(format!("could not build response: {error}")))
}

/// Read a file, honoring an optional `bytes=start-end` range so large media
/// can be streamed by the viewer without loading it whole.
type RawChunk = (Vec<u8>, u64, Option<(u64, u64)>);

pub fn read_raw(path: &Path, range: Option<&str>) -> AppResult<RawChunk> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path)?;
    let total = file.metadata()?.len();
    let parsed = range.and_then(|value| {
        let value = value.trim().strip_prefix("bytes=")?;
        let (start, end) = value.split_once('-')?;
        let start = start.trim().parse::<u64>().ok()?;
        let end = if end.trim().is_empty() {
            total.saturating_sub(1)
        } else {
            end.trim().parse::<u64>().ok()?.min(total.saturating_sub(1))
        };
        (start <= end && start < total).then_some((start, end))
    });
    if let Some((start, end)) = parsed {
        file.seek(SeekFrom::Start(start))?;
        let mut bytes = vec![0; (end - start + 1) as usize];
        file.read_exact(&mut bytes)?;
        Ok((bytes, total, Some((start, end))))
    } else {
        if total > 250 * 1024 * 1024 {
            return Err(AppError::BadRequest(
                "file is too large for the in-app viewer".into(),
            ));
        }
        let mut bytes = Vec::with_capacity(total as usize);
        file.read_to_end(&mut bytes)?;
        Ok((bytes, total, None))
    }
}

pub fn mime_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" | "cjs" => "text/javascript; charset=utf-8",
        "json" | "jsonc" => "application/json; charset=utf-8",
        "md" | "markdown" | "mdx" => "text/markdown; charset=utf-8",
        "txt" | "log" | "jsonl" | "csv" | "tsv" | "yaml" | "yml" | "toml" => {
            "text/plain; charset=utf-8"
        }
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "pdf" => "application/pdf",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "m4a" => "audio/mp4",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> tempfile::TempDir {
        let temp = tempfile::TempDir::new().expect("temp");
        std::fs::create_dir_all(temp.path().join("input")).expect("input");
        std::fs::write(temp.path().join("input/brief.md"), "# Brief\n").expect("file");
        temp
    }

    #[test]
    fn resolves_relative_paths_inside_the_workspace() {
        let temp = workspace();
        let root = std::fs::canonicalize(temp.path()).expect("root");
        let path = resolve_path(&root, Some("input/brief.md")).expect("path");
        assert!(path.ends_with("input/brief.md"));
    }

    #[test]
    fn rejects_path_traversal_and_absolute_paths() {
        let temp = workspace();
        let root = std::fs::canonicalize(temp.path()).expect("root");
        assert!(resolve_path(&root, Some("../etc/passwd")).is_err());
        assert!(resolve_path(&root, Some("/etc/passwd")).is_err());
        assert!(resolve_path(&root, Some("input\\brief.md")).is_err());
    }

    #[test]
    fn skips_heavy_directories() {
        assert!(should_skip(".git"));
        assert!(should_skip("node_modules"));
        assert!(should_skip("target"));
        assert!(!should_skip("input"));
    }
}
