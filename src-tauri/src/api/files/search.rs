use crate::{app_state::AppState, error::AppResult};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};

use super::paths::{FileQuery, relative_to, workspace_root};

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

pub(super) fn search_root(root: &Path, needle: &str, limit: usize) -> Vec<FileSearchResult> {
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
