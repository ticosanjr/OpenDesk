use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
    opencode_client::{
        daemon_manager::DaemonStatus,
        updater::{self, OpenCodeUpdateCheck},
    },
};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

use super::lifecycle::{EngineStatus, status};

const RESTART_TIMEOUT: Duration = Duration::from_secs(45);
const UPDATE_TIMEOUT: Duration = Duration::from_secs(600);

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

    let mut command = tokio::process::Command::new(&binary.path);
    command
        .args(["upgrade", "--method", &method, &version])
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let result = tokio::time::timeout(UPDATE_TIMEOUT, command.output()).await;

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
