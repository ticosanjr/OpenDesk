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
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

const MAX_PREVIEW_BYTES: usize = 512 * 1024;
const MAX_ENTRIES: usize = 400;

#[derive(Debug, Deserialize, Default)]
pub struct FileQuery {
    pub workspace_id: Option<String>,
    pub path: Option<String>,
    pub range: Option<String>,
    pub q: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileSearchResult {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<String>,
}

type SearchIndex = HashMap<PathBuf, (Instant, Vec<FileSearchResult>)>;
static SEARCH_INDEX: OnceLock<Mutex<SearchIndex>> = OnceLock::new();
const SEARCH_INDEX_TTL: Duration = Duration::from_secs(15);
const SEARCH_INDEX_MAX: usize = 20_000;

const SEARCH_SKIP: [&str; 8] = [
    ".git",
    "node_modules",
    "target",
    "dist",
    ".vite",
    ".cache",
    "releases",
    "coverage",
];

/// Recursive name search across the whole workspace. Heavy build folders are
/// skipped so typing in the search box never walks a Rust/Node tree.
pub async fn search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileQuery>,
) -> AppResult<Json<Vec<FileSearchResult>>> {
    let root = workspace_root(&state, query.workspace_id.as_deref()).await?;
    let needle = query.q.as_deref().unwrap_or("").trim().to_lowercase();
    if needle.len() < 2 {
        return Ok(Json(Vec::new()));
    }
    let limit = query.limit.unwrap_or(60).clamp(1, 200);
    Ok(Json(search_root(&root, &needle, limit)))
}

fn build_index(root: &Path) -> Vec<FileSearchResult> {
    let mut results = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .max_depth(12)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            let name = entry.file_name().to_string_lossy();
            !SEARCH_SKIP.iter().any(|skip| *skip == name)
        })
        .flatten()
    {
        if entry.path() == root {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(metadata) if !metadata.file_type().is_symlink() => metadata,
            _ => continue,
        };
        results.push(FileSearchResult {
            name: entry.file_name().to_string_lossy().to_string(),
            path: relative_to(root, entry.path()),
            is_dir: metadata.is_dir(),
            size: metadata.len(),
            modified: metadata
                .modified()
                .ok()
                .map(|value| chrono::DateTime::<chrono::Utc>::from(value).to_rfc3339()),
        });
        if results.len() >= SEARCH_INDEX_MAX {
            break;
        }
    }
    results
}

fn search_root(root: &Path, needle: &str, limit: usize) -> Vec<FileSearchResult> {
    let cache = SEARCH_INDEX.get_or_init(|| Mutex::new(HashMap::new()));
    let cached = cache.lock().ok().and_then(|guard| {
        guard
            .get(root)
            .filter(|(at, _)| at.elapsed() < SEARCH_INDEX_TTL)
            .map(|(_, value)| value.clone())
    });
    let entries = match cached {
        Some(value) => value,
        None => {
            // Build outside the lock so one large workspace does not block
            // searches for other workspaces.
            let index = build_index(root);
            if let Ok(mut guard) = cache.lock() {
                guard.insert(root.to_path_buf(), (Instant::now(), index.clone()));
            }
            index
        }
    };
    let mut results: Vec<FileSearchResult> = entries
        .into_iter()
        .filter(|entry| entry.path.to_lowercase().contains(needle))
        .collect();
    results.sort_by_key(|entry| {
        let name = entry.name.to_lowercase();
        (
            if name.contains(needle) { 0 } else { 1 },
            if name.starts_with(needle) { 0 } else { 1 },
            if entry.is_dir { 0 } else { 1 },
            entry.path.len(),
        )
    });
    results.truncate(limit);
    results
}

pub fn invalidate_search_index() {
    if let Some(cache) = SEARCH_INDEX.get()
        && let Ok(mut guard) = cache.lock()
    {
        guard.clear();
    }
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

/// Read technical metadata for video/audio assets with ffprobe. This gives
/// agents and the viewer duration, codecs and resolution without decoding.
pub async fn media(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let root = workspace_root(&state, query.workspace_id.as_deref()).await?;
    let path = resolve_path(&root, query.path.as_deref())?;
    if !path.is_file() {
        return Err(AppError::BadRequest("path is not a file".into()));
    }
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        tokio::process::Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
            ])
            .arg(&path)
            .output(),
    )
    .await
    .map_err(|_| AppError::Internal("ffprobe timed out".into()))?
    .map_err(|error| AppError::BadRequest(format!("ffprobe indisponível: {error}")))?;
    if !output.status.success() {
        return Err(AppError::BadRequest(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| AppError::Internal(format!("ffprobe output: {error}")))?;
    let format = value.get("format");
    let streams = value
        .get("streams")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|stream| {
                    serde_json::json!({
                        "type": stream.get("codec_type").and_then(serde_json::Value::as_str),
                        "codec": stream.get("codec_name").and_then(serde_json::Value::as_str),
                        "width": stream.get("width").and_then(serde_json::Value::as_u64),
                        "height": stream.get("height").and_then(serde_json::Value::as_u64),
                        "channels": stream.get("channels").and_then(serde_json::Value::as_u64),
                        "sample_rate": stream.get("sample_rate").and_then(serde_json::Value::as_str),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(Json(serde_json::json!({
        "path": relative_to(&root, &path),
        "format": format.and_then(|value| value.get("format_long_name")).and_then(serde_json::Value::as_str),
        "duration_secs": format.and_then(|value| value.get("duration")).and_then(serde_json::Value::as_str).and_then(|value| value.parse::<f64>().ok()),
        "bit_rate": format.and_then(|value| value.get("bit_rate")).and_then(serde_json::Value::as_str).and_then(|value| value.parse::<u64>().ok()),
        "streams": streams,
    })))
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
    fn searches_recursively_and_ignores_node_modules() {
        let temp = workspace();
        std::fs::create_dir_all(temp.path().join("output/2026-11/os")).expect("output");
        std::fs::write(temp.path().join("output/2026-11/os/relatorio.docx"), b"x").expect("report");
        std::fs::create_dir_all(temp.path().join("node_modules/pkg")).expect("nm");
        std::fs::write(temp.path().join("node_modules/pkg/relatorio.js"), b"x").expect("nm file");
        let root = std::fs::canonicalize(temp.path()).expect("root");
        let results = search_root(&root, "relatorio", 50);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, "output/2026-11/os/relatorio.docx");
    }

    #[test]
    fn skips_heavy_directories() {
        assert!(should_skip(".git"));
        assert!(should_skip("node_modules"));
        assert!(should_skip("target"));
        assert!(!should_skip("input"));
    }
}
