use crate::{app_state::AppState, error::AppResult, updates};
use axum::{Json, extract::State};
use serde_json::{Value, json};
use std::{path::Path, process::Command, sync::Arc};

pub async fn about(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let daemon = state.daemon.status().await;
    let resolved = state.locator.resolve().ok();
    let configured = state
        .locator
        .configured()
        .map(|path| path.to_string_lossy().to_string());
    let schema_version: Option<i64> =
        sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
            .fetch_one(&state.db)
            .await
            .ok();
    let workspaces: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    let db_size = std::fs::metadata(&state.paths.db_path)
        .map(|meta| meta.len())
        .unwrap_or(0);

    Ok(Json(json!({
        "app": {
            "name": "OpenDesk",
            "version": env!("CARGO_PKG_VERSION"),
            "phase": "resilience",
            "api": "OpenCode V2 (/api/*)",
        },
        "runtime": {
            "tauri": tauri::VERSION,
            "rustc": command_version("rustc"),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "kernel": kernel_version(),
        },
        "engine": {
            "daemon": daemon,
            "configured_binary": configured,
            "resolved": resolved,
            "installer": updates::installer_kind(),
        },
        "database": {
            "path": state.paths.db_path,
            "size_bytes": db_size,
            "schema_version": schema_version,
            "workspaces": workspaces,
        },
        "paths": {
            "data_dir": state.paths.data_dir,
            "config_dir": state.paths.config_dir,
            "log_dir": state.paths.log_dir,
            "db_path": state.paths.db_path,
            "backups_dir": state.paths.backups_dir,
            "run_logs_dir": state.paths.run_logs_dir,
        },
        "updates": {
            "current": updates::current_version(),
            "installer": updates::installer_kind(),
            "token_configured": updates::token(&state.secrets).is_some(),
        },
        "logs": log_files(&state.paths.log_dir),
        "started_at": state.started_at,
        "api_port": state.api_port,
    })))
}

fn command_version(program: &str) -> Option<String> {
    let output = Command::new(program).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn kernel_version() -> Option<String> {
    #[cfg(unix)]
    {
        let output = Command::new("uname").arg("-r").output().ok()?;
        if !output.status.success() {
            return None;
        }
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn log_files(dir: &Path) -> Vec<Value> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            Some(json!({
                "name": path.file_name().map(|name| name.to_string_lossy().to_string()),
                "path": path,
                "size_bytes": metadata.len(),
                "modified": metadata.modified().ok().map(|value| {
                    chrono::DateTime::<chrono::Utc>::from(value).to_rfc3339()
                }),
            }))
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| {
        right
            .get("modified")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .cmp(
                left.get("modified")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            )
    });
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_version_is_reported_on_unix() {
        #[cfg(unix)]
        assert!(kernel_version().is_some());
    }

    #[test]
    fn log_files_are_sorted_and_sized() {
        let temp = tempfile::TempDir::new().expect("temp");
        std::fs::write(temp.path().join("a.log"), "hello").expect("log");
        let files = log_files(temp.path());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0]["size_bytes"], 5);
    }
}
