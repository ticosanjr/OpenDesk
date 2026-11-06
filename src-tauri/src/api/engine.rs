use crate::{
    app_state::{AppState, OPENCODE_BIN_SETTING},
    db::{settings, workspaces},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
    opencode_client::{
        binary::OpenCodeBinaryInfo,
        daemon_manager::DaemonStatus,
        updater::{self, OpenCodeUpdateCheck},
    },
};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc, time::Duration};

const RESTART_TIMEOUT: Duration = Duration::from_secs(45);
const UPDATE_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Serialize)]
pub struct EngineStatus {
    pub daemon: DaemonStatus,
    pub configured_binary: Option<String>,
    pub resolved: Option<OpenCodeBinaryInfo>,
    pub resolve_error: Option<String>,
    pub candidates: Vec<String>,
    pub app_version: &'static str,
}

#[derive(Debug, Serialize)]
pub struct DetectResult {
    pub ok: bool,
    pub binary: Option<OpenCodeBinaryInfo>,
    pub error: Option<String>,
    pub candidates: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct BinaryBody {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBody {
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub method: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpdateResult {
    pub ok: bool,
    pub version: Option<String>,
    pub output: String,
    pub daemon: DaemonStatus,
}

/// Return an actionable 503 when the engine is offline so dispatch and
/// "run now" never enqueue work that is guaranteed to fail.
pub async fn require_engine(state: &AppState) -> AppResult<()> {
    // Headless test mode deliberately disables the engine; the UI/queue tests
    // still need to enqueue work. Only honor the bypass in debug builds so a
    // stray environment variable can never make a release enqueue doomed jobs.
    if cfg!(debug_assertions) && std::env::var("OPENDESK_SKIP_DAEMON").is_ok() {
        return Ok(());
    }
    require_engine_strict(state).await
}

/// Same check without the headless-test bypass. Used by Kanban transitions,
/// where the test must prove that a failed dispatch keeps the chosen column.
pub async fn require_engine_strict(state: &AppState) -> AppResult<()> {
    let status = state.daemon.status().await;
    if status.online {
        return Ok(());
    }

    let mut message = String::from(
        "OpenCode is offline, so this action was not queued. Open Settings → OpenCode Engine to fix it.",
    );
    if let Some(reason) = status.message {
        message.push_str(" Reason: ");
        message.push_str(&reason);
    } else {
        message.push_str(" No OpenCode binary was detected and no shared service is reachable.");
    }
    Err(AppError::Daemon(message))
}

pub async fn status(State(state): State<Arc<AppState>>) -> Json<EngineStatus> {
    let daemon = state.daemon.status().await;
    let locator = state.locator.clone();
    let configured_binary = state
        .locator
        .configured()
        .map(|path| path.to_string_lossy().to_string());
    let (resolved, resolve_error) = match locator.resolve() {
        Ok(binary) => (Some(binary), None),
        Err(error) => (None, Some(format!("{error:#}"))),
    };
    let candidates = locator
        .candidates()
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();

    Json(EngineStatus {
        daemon,
        configured_binary,
        resolved,
        resolve_error,
        candidates,
        app_version: env!("CARGO_PKG_VERSION"),
    })
}

pub async fn detect(State(state): State<Arc<AppState>>) -> Json<DetectResult> {
    let locator = state.locator.clone();
    let candidates = locator
        .candidates()
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    match locator.resolve() {
        Ok(binary) => Json(DetectResult {
            ok: true,
            binary: Some(binary),
            error: None,
            candidates,
        }),
        Err(error) => Json(DetectResult {
            ok: false,
            binary: None,
            error: Some(format!("{error:#}")),
            candidates,
        }),
    }
}

pub async fn set_binary(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BinaryBody>,
) -> AppResult<Json<EngineStatus>> {
    let raw = body.path.trim();
    if raw.is_empty() {
        return Err(AppError::BadRequest(
            "Informe o caminho completo do binário opencode.".into(),
        ));
    }
    let path = PathBuf::from(raw);
    let info = state.locator.resolve_path(&path)?;

    settings::set(
        &state.db,
        OPENCODE_BIN_SETTING,
        &Value::String(info.path.clone()),
    )
    .await?;
    state.locator.set_configured(Some(path));
    state.daemon.restart().await;
    let _ = state.daemon.wait_for_settled(RESTART_TIMEOUT).await;

    Ok(status(State(state)).await)
}

pub async fn restart(State(state): State<Arc<AppState>>) -> Json<EngineStatus> {
    state.daemon.restart().await;
    let _ = state.daemon.wait_for_settled(RESTART_TIMEOUT).await;
    status(State(state)).await
}

pub async fn update_check(State(state): State<Arc<AppState>>) -> Json<OpenCodeUpdateCheck> {
    let current = state
        .daemon
        .status()
        .await
        .version
        .or_else(|| state.locator.resolve().ok().map(|binary| binary.version));
    let client = match updater::http_client() {
        Ok(client) => client,
        Err(error) => {
            return Json(OpenCodeUpdateCheck {
                current,
                latest: None,
                update_available: false,
                release: None,
                checked_at: chrono::Utc::now().to_rfc3339(),
                error: Some(format!("{error:#}")),
            });
        }
    };

    match updater::fetch_latest(&client).await {
        Ok(release) => {
            let available = updater::update_available(current.as_deref(), &release.version);
            Json(OpenCodeUpdateCheck {
                current,
                latest: Some(release.version.clone()),
                update_available: available,
                release: Some(release),
                checked_at: chrono::Utc::now().to_rfc3339(),
                error: None,
            })
        }
        Err(error) => Json(OpenCodeUpdateCheck {
            current,
            latest: None,
            update_available: false,
            release: None,
            checked_at: chrono::Utc::now().to_rfc3339(),
            error: Some(format!("{error:#}")),
        }),
    }
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateBody>,
) -> AppResult<Json<UpdateResult>> {
    let status = state.daemon.status().await;
    if !status.online {
        // Updating while offline is allowed: it is one of the ways to fix a
        // broken install. The current binary still needs to be resolvable.
    }

    let running: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM execution_queue WHERE status = 'RUNNING'")
            .fetch_one(&state.db)
            .await?;
    if running > 0 {
        return Err(AppError::Conflict(
            "Há jobs em execução. Aguarde terminar ou cancele antes de atualizar o OpenCode."
                .into(),
        ));
    }

    let binary = state.locator.resolve()?;
    let backup_path = std::path::PathBuf::from(format!("{}.bak", binary.path));
    if let Err(error) = std::fs::copy(&binary.path, &backup_path) {
        tracing::warn!(error = %error, path = %backup_path.display(), "could not back up the OpenCode binary");
    }
    let client = updater::http_client()?;

    let version = match body.version {
        Some(version) if !version.trim().is_empty() => version.trim().to_string(),
        _ => updater::fetch_latest(&client).await?.version,
    };

    let method = body
        .method
        .filter(|value| !value.trim().is_empty())
        .or_else(|| binary.method.clone());
    let Some(method) = method else {
        return Err(AppError::BadRequest(
            "Não foi possível detectar o método de instalação. Escolha curl, npm, bun ou brew."
                .into(),
        ));
    };

    let was_paused = state.queue.is_paused();
    if !was_paused {
        state
            .queue
            .pause("paused for the OpenCode update".to_string())
            .await;
    }

    state.events.publish(OpenDeskWsEvent::EngineUpdateProgress {
        phase: "running".into(),
        message: Some(format!("updating OpenCode to {version} using {method}")),
        ok: None,
    });

    let result = tokio::time::timeout(
        UPDATE_TIMEOUT,
        tokio::process::Command::new(&binary.path)
            .args(["upgrade", "--method", &method, &version])
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .output(),
    )
    .await;

    let (ok, output) = match result {
        Ok(Ok(output)) => {
            let mut text = String::from_utf8_lossy(&output.stdout).to_string();
            text.push_str(&String::from_utf8_lossy(&output.stderr));
            (output.status.success(), text)
        }
        Ok(Err(error)) => (false, format!("could not run `opencode upgrade`: {error}")),
        Err(_) => (
            false,
            format!(
                "`opencode upgrade` excedeu {} segundos",
                UPDATE_TIMEOUT.as_secs()
            ),
        ),
    };

    if !was_paused {
        let _ = state.queue.resume().await;
    }

    state.daemon.restart().await;
    let daemon = state.daemon.wait_for_settled(RESTART_TIMEOUT).await;
    let installed = state.locator.resolve().ok();

    let ok = ok
        && daemon.online
        && installed
            .as_ref()
            .map(|binary| !updater::update_available(Some(&binary.version), &version))
            .unwrap_or(false);

    state.events.publish(OpenDeskWsEvent::EngineUpdateProgress {
        phase: if ok { "done".into() } else { "failed".into() },
        message: Some(if ok {
            format!("OpenCode atualizado para {version}")
        } else {
            "A atualização falhou; o daemon não confirma a nova versão".into()
        }),
        ok: Some(ok),
    });

    Ok(Json(UpdateResult {
        ok,
        version: installed.map(|binary| binary.version),
        output,
        daemon,
    }))
}

pub async fn rollback(State(state): State<Arc<AppState>>) -> AppResult<Json<EngineStatus>> {
    let binary = state.locator.resolve()?;
    let backup = std::path::PathBuf::from(format!("{}.bak", binary.path));
    if !backup.is_file() {
        return Err(AppError::BadRequest(format!(
            "não há backup do OpenCode anterior em {}",
            backup.display()
        )));
    }
    let staged = std::path::PathBuf::from(format!("{}.new", binary.path));
    std::fs::copy(&backup, &staged)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&staged, &binary.path)?;
    state.daemon.restart().await;
    let _ = state.daemon.wait_for_settled(RESTART_TIMEOUT).await;
    Ok(status(State(state)).await)
}

pub async fn model_health(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let entries = crate::db::model_health::list(&state.db, 24).await?;
    Ok(Json(json!({ "models": entries })))
}

#[derive(Debug, serde::Deserialize)]
pub struct ClearModelHealthBody {
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
}

pub async fn clear_model_health(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ClearModelHealthBody>,
) -> AppResult<Json<Value>> {
    if let (Some(provider_id), Some(model_id)) = (body.provider_id, body.model_id) {
        crate::db::model_health::clear_model(&state.db, &provider_id, &model_id).await?;
    } else {
        crate::db::model_health::clear(&state.db).await?;
    }
    model_health(State(state)).await
}

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

fn tail(path: &std::path::Path, lines: usize) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_reads_only_the_requested_lines() {
        let temp = tempfile::TempDir::new().expect("temp");
        let path = temp.path().join("opendesk.log");
        let content = (0..1000)
            .map(|index| format!("line-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&path, content).expect("log");

        let value = tail(&path, 5);
        let lines = value.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines.last().copied(), Some("line-999"));
    }

    #[test]
    fn tail_returns_empty_for_missing_files() {
        assert_eq!(tail(std::path::Path::new("/nope/missing.log"), 10), "");
    }
}
