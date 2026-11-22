//! Computer-use bridge.
//!
//! This is deliberately a thin, auditable executor over host tools
//! (`grim`/`import` for screenshots; `xdotool`/`ydotool` for input). It never
//! embeds a screen stream or bypasses the OS: on Wayland input depends on
//! `ydotool` having its daemon running. Every action is counted and summarized
//! without storing typed text.

use crate::{
    db::settings,
    error::{AppError, AppResult},
    paths::AppPaths,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::{path::Path, time::Duration};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct ComputerCapabilities {
    pub available: bool,
    pub backend: String,
    pub display: Option<String>,
    pub screenshot_tool: Option<String>,
    pub input_tool: Option<String>,
    pub reason: Option<String>,
}

pub fn capabilities() -> ComputerCapabilities {
    capabilities_for(None)
}

/// Capabilities for a new session, including the Xvfb requirement when the
/// session should not touch the user's desktop.
pub fn can_start(isolated: bool) -> ComputerCapabilities {
    if !isolated {
        return capabilities();
    }
    let mut capabilities = capabilities_for(Some(":virtual"));
    if !crate::virtual_display::available() {
        capabilities.available = false;
        capabilities.reason =
            Some("display isolado exige Xvfb, xdotool e imagemagick (import)".into());
    }
    capabilities
}

pub fn capabilities_for(forced_display: Option<&str>) -> ComputerCapabilities {
    if let Some(display) = forced_display {
        let screenshot = ["import", "scrot"]
            .into_iter()
            .find(|command| command_exists(command));
        let input = ["xdotool", "ydotool"]
            .into_iter()
            .find(|command| command_exists(command));
        return ComputerCapabilities {
            available: screenshot.is_some() && input.is_some(),
            backend: "x11-isolated".into(),
            display: Some(display.to_string()),
            screenshot_tool: screenshot.map(str::to_string),
            input_tool: input.map(str::to_string),
            reason: if screenshot.is_none() {
                Some("instale imagemagick (import) para capturas no display virtual".into())
            } else if input.is_none() {
                Some("instale xdotool para entrada no display virtual".into())
            } else {
                None
            },
        };
    }
    let display = std::env::var("WAYLAND_DISPLAY")
        .ok()
        .map(|value| format!("wayland:{value}"))
        .or_else(|| {
            std::env::var("DISPLAY")
                .ok()
                .map(|value| format!("x11:{value}"))
        });
    let screenshot = ["grim", "import", "scrot"]
        .into_iter()
        .find(|command| command_exists(command));
    let input = ["ydotool", "xdotool"]
        .into_iter()
        .find(|command| command_exists(command));
    let backend = if display
        .as_deref()
        .is_some_and(|value| value.starts_with("wayland"))
    {
        "wayland"
    } else if display.is_some() {
        "x11"
    } else {
        "none"
    };
    let available = display.is_some() && screenshot.is_some() && input.is_some();
    let reason = if display.is_none() {
        Some("sessão gráfica não detectada (DISPLAY/WAYLAND_DISPLAY ausente)".into())
    } else if screenshot.is_none() {
        Some("instale grim (Wayland) ou imagemagick/scrot (X11) para capturas".into())
    } else if input.is_none() {
        Some("instale ydotool (Wayland) ou xdotool (X11) para entrada".into())
    } else {
        None
    };
    ComputerCapabilities {
        available,
        backend: backend.into(),
        display,
        screenshot_tool: screenshot.map(str::to_string),
        input_tool: input.map(str::to_string),
        reason,
    }
}

fn command_exists(command: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(command).is_file()))
        .unwrap_or(false)
}

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

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ComputerSession {
    pub id: String,
    pub workspace_id: String,
    pub run_id: Option<String>,
    pub status: String,
    pub require_approval: bool,
    pub actions: i64,
    pub last_action_json: Option<String>,
    pub screenshot_path: Option<String>,
    #[sqlx(default)]
    pub display: Option<String>,
    #[sqlx(default)]
    pub isolated: bool,
    pub created_at: String,
    pub ended_at: Option<String>,
}

const COLUMNS: &str = "id, workspace_id, run_id, status, require_approval, actions, \
    last_action_json, screenshot_path, display, isolated, created_at, ended_at";

pub async fn create(
    pool: &SqlitePool,
    workspace_id: &str,
    run_id: Option<&str>,
    require_approval: bool,
    isolated: bool,
) -> AppResult<ComputerSession> {
    let id = Uuid::now_v7().to_string();
    let status = if require_approval {
        "PENDING"
    } else {
        "ACTIVE"
    };
    let display = if isolated {
        Some(crate::virtual_display::start(&id)?)
    } else {
        None
    };
    let inserted = sqlx::query(
        "INSERT INTO computer_sessions
            (id, workspace_id, run_id, status, require_approval, display, isolated)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(run_id)
    .bind(status)
    .bind(require_approval)
    .bind(&display)
    .bind(isolated)
    .execute(pool)
    .await;
    if let Err(error) = inserted {
        crate::virtual_display::stop(&id);
        return Err(error.into());
    }
    get(pool, &id).await
}

pub async fn list(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<ComputerSession>> {
    let query = format!(
        "SELECT {COLUMNS} FROM computer_sessions WHERE workspace_id = ?
         ORDER BY created_at DESC LIMIT 50"
    );
    Ok(sqlx::query_as::<_, ComputerSession>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<ComputerSession> {
    let query = format!("SELECT {COLUMNS} FROM computer_sessions WHERE id = ?");
    sqlx::query_as::<_, ComputerSession>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("computer session {id}")))
}

pub async fn approve(pool: &SqlitePool, id: &str) -> AppResult<ComputerSession> {
    let session = get(pool, id).await?;
    if session.status == "ENDED" {
        return Err(AppError::Conflict("sessão encerrada".into()));
    }
    if session.status == "PENDING" {
        sqlx::query("UPDATE computer_sessions SET status = 'ACTIVE' WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
    }
    get(pool, id).await
}

pub async fn end(pool: &SqlitePool, id: &str) -> AppResult<ComputerSession> {
    sqlx::query(
        "UPDATE computer_sessions SET status = 'ENDED',
                ended_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(id)
    .execute(pool)
    .await?;
    crate::virtual_display::stop(id);
    get(pool, id).await
}

async fn record_action(pool: &SqlitePool, id: &str, summary: serde_json::Value) -> AppResult<()> {
    sqlx::query(
        "UPDATE computer_sessions
         SET actions = actions + 1, last_action_json = ?
         WHERE id = ?",
    )
    .bind(summary.to_string())
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn screenshot(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_root: &Path,
    id: &str,
) -> AppResult<(String, Vec<u8>)> {
    let session = get(pool, id).await?;
    require_active(&session)?;
    let capabilities = capabilities_for(session.display.as_deref());
    ensure_window_allowed(pool, &session, &capabilities).await?;
    let Some(tool) = capabilities.screenshot_tool.as_deref() else {
        return Err(AppError::Conflict("nenhuma ferramenta de captura".into()));
    };
    let dir = workspace_root
        .join("output")
        .join(chrono::Utc::now().format("%Y-%m").to_string())
        .join("computer");
    tokio::fs::create_dir_all(&dir).await?;
    let file = dir.join(format!("{id}.png"));
    let mut command = tokio::process::Command::new(tool);
    command.kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    if tool == "import" {
        command.args(["-window", "root"]);
    }
    command.arg(&file);
    if let Some(display) = session.display.as_deref() {
        command.env("DISPLAY", display);
    }
    let output = tokio::time::timeout(Duration::from_secs(15), command.output())
        .await
        .map_err(|_| AppError::Internal("captura expirou".into()))??;
    if !output.status.success() {
        return Err(AppError::Internal(format!(
            "{tool} falhou: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .next()
                .unwrap_or("")
        )));
    }
    let bytes = tokio::fs::read(&file).await?;
    let path = file.to_string_lossy().to_string();
    sqlx::query("UPDATE computer_sessions SET screenshot_path = ? WHERE id = ?")
        .bind(&path)
        .bind(id)
        .execute(pool)
        .await?;
    record_action(pool, id, serde_json::json!({ "type": "screenshot" })).await?;
    let _ = paths;
    Ok((path, bytes))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ComputerAction {
    Click {
        x: i64,
        y: i64,
        #[serde(default)]
        button: Option<String>,
    },
    Move {
        x: i64,
        y: i64,
    },
    Type {
        text: String,
    },
    Key {
        key: String,
    },
    Scroll {
        amount: i64,
    },
}

pub async fn act(
    pool: &SqlitePool,
    id: &str,
    action: ComputerAction,
) -> AppResult<serde_json::Value> {
    let session = get(pool, id).await?;
    require_active(&session)?;
    let capabilities = capabilities_for(session.display.as_deref());
    ensure_window_allowed(pool, &session, &capabilities).await?;
    let Some(tool) = capabilities.input_tool.clone() else {
        return Err(AppError::Conflict("nenhuma ferramenta de entrada".into()));
    };
    let (program, args, summary) = build_action(&capabilities, &tool, &action)?;
    let mut command = tokio::process::Command::new(&program);
    command.args(&args).kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    if let Some(display) = session.display.as_deref() {
        command.env("DISPLAY", display);
    }
    let output = tokio::time::timeout(Duration::from_secs(15), command.output())
        .await
        .map_err(|_| AppError::Internal("ação expirou".into()))??;
    if !output.status.success() {
        return Err(AppError::Internal(format!(
            "{tool} falhou: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .next()
                .unwrap_or("")
        )));
    }
    record_action(pool, id, summary.clone()).await?;
    Ok(summary)
}

async fn ensure_window_allowed(
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

fn require_active(session: &ComputerSession) -> AppResult<()> {
    match session.status.as_str() {
        "ACTIVE" => Ok(()),
        "PENDING" => Err(AppError::Conflict("sessão aguarda aprovação humana".into())),
        _ => Err(AppError::Conflict("sessão encerrada".into())),
    }
}

fn build_action(
    capabilities: &ComputerCapabilities,
    tool: &str,
    action: &ComputerAction,
) -> AppResult<(String, Vec<String>, serde_json::Value)> {
    let wayland = capabilities.backend == "wayland";
    match action {
        ComputerAction::Click { x, y, button } => {
            let button = match button.as_deref().unwrap_or("left") {
                "right" => "3",
                "middle" => "2",
                _ => "1",
            };
            if !(0..=20_000).contains(x) || !(0..=20_000).contains(y) {
                return Err(AppError::BadRequest("coordenadas fora do limite".into()));
            }
            if wayland {
                Ok((
                    tool.into(),
                    vec![
                        "mousemove".into(),
                        "-a".into(),
                        x.to_string(),
                        y.to_string(),
                    ],
                    serde_json::json!({ "type": "click", "x": x, "y": y }),
                ))
            } else {
                Ok((
                    tool.into(),
                    vec![
                        "mousemove".into(),
                        x.to_string(),
                        y.to_string(),
                        "click".into(),
                        button.into(),
                    ],
                    serde_json::json!({ "type": "click", "x": x, "y": y }),
                ))
            }
        }
        ComputerAction::Move { x, y } => {
            if !(0..=20_000).contains(x) || !(0..=20_000).contains(y) {
                return Err(AppError::BadRequest("coordenadas fora do limite".into()));
            }
            let args = if wayland {
                vec![
                    "mousemove".into(),
                    "-a".into(),
                    x.to_string(),
                    y.to_string(),
                ]
            } else {
                vec!["mousemove".into(), x.to_string(), y.to_string()]
            };
            Ok((
                tool.into(),
                args,
                serde_json::json!({ "type": "move", "x": x, "y": y }),
            ))
        }
        ComputerAction::Type { text } => {
            let text = text.trim_end_matches('\0');
            if text.len() > 4000 {
                return Err(AppError::BadRequest("texto grande demais".into()));
            }
            let args = if wayland {
                vec!["type".into(), "--".into(), text.to_string()]
            } else {
                vec![
                    "type".into(),
                    "--delay".into(),
                    "20".into(),
                    "--".into(),
                    text.to_string(),
                ]
            };
            // Never persist what was typed.
            Ok((
                tool.into(),
                args,
                serde_json::json!({ "type": "type", "length": text.chars().count() }),
            ))
        }
        ComputerAction::Key { key } => {
            let key = key.trim();
            if key.is_empty() || key.len() > 80 {
                return Err(AppError::BadRequest("tecla inválida".into()));
            }
            Ok((
                tool.into(),
                vec!["key".into(), key.to_string()],
                serde_json::json!({ "type": "key", "key": key }),
            ))
        }
        ComputerAction::Scroll { amount } => {
            let amount = (*amount).clamp(-20, 20);
            let args = if wayland {
                vec![
                    "click".into(),
                    if amount < 0 { "0x40" } else { "0x41" }.into(),
                ]
            } else {
                vec!["click".into(), "4".into()]
            };
            Ok((
                tool.into(),
                args,
                serde_json::json!({ "type": "scroll", "amount": amount }),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_x11_and_wayland_actions() {
        let x11 = ComputerCapabilities {
            available: true,
            backend: "x11".into(),
            display: Some("x11::0".into()),
            screenshot_tool: Some("import".into()),
            input_tool: Some("xdotool".into()),
            reason: None,
        };
        let (tool, args, summary) = build_action(
            &x11,
            "xdotool",
            &ComputerAction::Click {
                x: 10,
                y: 20,
                button: None,
            },
        )
        .expect("click");
        assert_eq!(tool, "xdotool");
        assert_eq!(args, vec!["mousemove", "10", "20", "click", "1"]);
        assert_eq!(summary["type"], "click");

        let wayland = ComputerCapabilities {
            backend: "wayland".into(),
            ..x11
        };
        let (_, args, _) = build_action(
            &wayland,
            "ydotool",
            &ComputerAction::Type {
                text: "segredo".into(),
            },
        )
        .expect("type");
        assert_eq!(args[0], "type");
        assert!(args.contains(&"segredo".to_string()));
    }

    #[test]
    fn rejects_out_of_range_coordinates() {
        let caps = ComputerCapabilities {
            available: true,
            backend: "x11".into(),
            display: Some("x11::0".into()),
            screenshot_tool: None,
            input_tool: Some("xdotool".into()),
            reason: None,
        };
        assert!(build_action(&caps, "xdotool", &ComputerAction::Move { x: 99_999, y: 0 }).is_err());
    }

    #[test]
    fn allowlist_matches_windows_and_apps_case_insensitively() {
        let allowlist = WindowAllowlist {
            enabled: true,
            windows: vec!["chromium".into()],
            apps: vec!["firefox".into()],
        }
        .normalize();
        assert!(allowlist.allows("Chromium — Notícias", "x"));
        assert!(allowlist.allows("Qualquer", "firefox"));
        assert!(!allowlist.allows("Terminal", "bash"));
        let disabled = WindowAllowlist {
            enabled: false,
            windows: vec![],
            apps: vec![],
        };
        assert!(disabled.allows("Terminal", "bash"));
    }

    #[tokio::test]
    async fn stores_sessions_and_requires_approval() {
        let ctx = crate::test_support::context().await;
        let session = create(&ctx.state.db, &ctx.workspace_id, None, true, false)
            .await
            .expect("session");
        assert_eq!(session.status, "PENDING");
        assert!(require_active(&session).is_err());
        let approved = approve(&ctx.state.db, &session.id).await.expect("approve");
        assert_eq!(approved.status, "ACTIVE");
        let ended = end(&ctx.state.db, &session.id).await.expect("end");
        assert_eq!(ended.status, "ENDED");
        assert_eq!(
            list(&ctx.state.db, &ctx.workspace_id)
                .await
                .expect("list")
                .len(),
            1
        );
    }
}
