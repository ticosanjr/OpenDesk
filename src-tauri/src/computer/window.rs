use crate::{
    db::settings,
    error::{AppError, AppResult},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::{ComputerCapabilities, ComputerSession};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WindowAllowlist {
    pub enabled: bool,
    /// Case-insensitive substrings matched against the active window title.
    pub windows: Vec<String>,
    /// Exact/contains matches against the process name.
    pub apps: Vec<String>,
}

impl WindowAllowlist {
    pub fn normalize(mut self) -> Self {
        self.windows = clean_list(self.windows);
        self.apps = clean_list(self.apps);
        self
    }

    pub fn allows(&self, window: &str, app: &str) -> bool {
        if !self.enabled {
            return true;
        }
        let window = window.to_lowercase();
        let app = app.to_lowercase();
        self.windows
            .iter()
            .any(|allowed| !allowed.trim().is_empty() && window.contains(&allowed.to_lowercase()))
            || self
                .apps
                .iter()
                .any(|allowed| !allowed.trim().is_empty() && app.contains(&allowed.to_lowercase()))
    }
}

fn clean_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .take(50)
        .collect()
}

pub async fn allowlist(pool: &SqlitePool) -> AppResult<WindowAllowlist> {
    Ok(settings::get(pool, "computer_allowlist")
        .await?
        .and_then(|value| serde_json::from_value::<WindowAllowlist>(value).ok())
        .unwrap_or_default()
        .normalize())
}

pub async fn set_allowlist(
    pool: &SqlitePool,
    allowlist: WindowAllowlist,
) -> AppResult<WindowAllowlist> {
    let allowlist = allowlist.normalize();
    settings::set(
        pool,
        "computer_allowlist",
        &serde_json::to_value(&allowlist).map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .await?;
    Ok(allowlist)
}

#[derive(Debug, Clone)]
struct ActiveWindow {
    title: String,
    app: String,
}

pub(super) async fn ensure_window_allowed(
    pool: &SqlitePool,
    session: &ComputerSession,
    capabilities: &ComputerCapabilities,
) -> AppResult<()> {
    let allowlist = allowlist(pool).await?;
    if !allowlist.enabled {
        return Ok(());
    }
    if capabilities.backend == "wayland" {
        return Err(AppError::Conflict(
            "allowlist de janela exige display X11; use uma sessão isolada".into(),
        ));
    }
    let active = active_window(session.display.as_deref()).await?;
    if allowlist.allows(&active.title, &active.app) {
        Ok(())
    } else {
        Err(AppError::Conflict(format!(
            "janela ativa fora da allowlist: '{}' ({})",
            active.title, active.app
        )))
    }
}

async fn active_window(display: Option<&str>) -> AppResult<ActiveWindow> {
    let mut title_command = tokio::process::Command::new("xdotool");
    title_command
        .args(["getactivewindow", "getwindowname"])
        .kill_on_drop(true);
    if let Some(display) = display {
        title_command.env("DISPLAY", display);
    }
    let title = title_command.output().await.map_err(|error| {
        AppError::Conflict(format!("não foi possível ler a janela ativa: {error}"))
    })?;
    if !title.status.success() {
        return Err(AppError::Conflict(
            "não foi possível ler a janela ativa (xdotool)".into(),
        ));
    }
    let title = String::from_utf8_lossy(&title.stdout).trim().to_string();

    let mut pid_command = tokio::process::Command::new("xdotool");
    pid_command
        .args(["getactivewindow", "getwindowpid"])
        .kill_on_drop(true);
    if let Some(display) = display {
        pid_command.env("DISPLAY", display);
    }
    let pid = pid_command.output().await.ok();
    let app = pid
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .and_then(|pid| std::fs::read_to_string(format!("/proc/{pid}/comm")).ok())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| "desconhecido".into());
    Ok(ActiveWindow { title, app })
}
