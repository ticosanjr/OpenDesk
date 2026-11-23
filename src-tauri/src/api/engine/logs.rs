use crate::{app_state::AppState, db::workspaces, error::AppResult};
use axum::{Json, extract::State};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};

pub async fn diagnostics(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let daemon = state.daemon.status().await;
    let configured_binary = state
        .locator
        .configured()
        .map(|path| path.to_string_lossy().to_string());
    let candidates = state
        .locator
        .candidates()
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    let resolved = state.locator.resolve().ok();
    let service_files = service_state_files();
    let workspaces = workspaces::list(&state.db).await?;

    let log_files = log_files(&state.paths.log_dir);
    let log_tail = log_files
        .first()
        .map(|(_, path)| tail(path, 200))
        .unwrap_or_default();

    Ok(Json(json!({
        "app": {
            "version": env!("CARGO_PKG_VERSION"),
            "api_port": state.api_port,
            "started_at": state.started_at,
            "data_dir": state.paths.data_dir,
            "config_dir": state.paths.config_dir,
            "log_dir": state.paths.log_dir,
        },
        "engine": {
            "daemon": daemon,
            "configured_binary": configured_binary,
            "resolved": resolved,
            "candidates": candidates,
            "service_state_files": service_files,
        },
        "workspaces": workspaces.len(),
        "logs": log_files.iter().map(|(name, path)| json!({
            "name": name,
            "path": path,
        })).collect::<Vec<_>>(),
        "log_tail": log_tail,
    })))
}

fn service_state_files() -> Vec<Value> {
    let mut files = Vec::new();
    let mut candidates = Vec::new();
    if let Some(state) = std::env::var_os("XDG_STATE_HOME") {
        candidates.push(PathBuf::from(state).join("opencode/service.json"));
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        candidates.push(home.join(".local/state/opencode/service.json"));
        candidates.push(home.join(".config/opencode/service.json"));
    }

    for path in candidates {
        if !path.is_file() {
            continue;
        }
        let value = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
        // Never expose the shared service password.
        files.push(json!({
            "path": path,
            "url": value.as_ref().and_then(|value| value.get("url")).cloned(),
            "pid": value.as_ref().and_then(|value| value.get("pid")).cloned(),
            "version": value.as_ref().and_then(|value| value.get("version")).cloned(),
            "has_password": value.as_ref().and_then(|value| value.get("password")).is_some(),
        }));
    }
    files
}

fn log_files(dir: &std::path::Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let modified = entry.metadata().ok()?.modified().ok()?;
            Some((modified, path))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    files
        .into_iter()
        .map(|(_, path)| {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_default();
            (name, path)
        })
        .collect()
}

pub(super) fn tail(path: &std::path::Path, lines: usize) -> String {
    use std::io::{Read, Seek, SeekFrom};

    const MAX_BYTES: u64 = 256 * 1024;
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let Ok(metadata) = file.metadata() else {
        return String::new();
    };
    let length = metadata.len();
    let start = length.saturating_sub(MAX_BYTES);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut buffer = Vec::with_capacity((length - start) as usize);
    if file.read_to_end(&mut buffer).is_err() {
        return String::new();
    }
    let mut text = String::from_utf8_lossy(&buffer).to_string();
    if start > 0 {
        // Drop the first partial line from the middle of the file.
        if let Some(index) = text.find('\n') {
            text.drain(..=index);
        }
    }
    let collected = text.lines().rev().take(lines).collect::<Vec<_>>();
    collected.into_iter().rev().collect::<Vec<_>>().join("\n")
}
