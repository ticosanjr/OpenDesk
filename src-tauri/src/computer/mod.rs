//! Computer-use bridge.
//!
//! This is deliberately a thin, auditable executor over host tools
//! (`grim`/`import` for screenshots; `xdotool`/`ydotool` for input). It never
//! embeds a screen stream or bypasses the OS: on Wayland input depends on
//! `ydotool` having its daemon running. Every action is counted and summarized
//! without storing typed text.

mod actions;
mod display;
#[cfg(test)]
mod tests;
mod window;

use crate::{
    error::{AppError, AppResult},
    paths::AppPaths,
};
use actions::build_action;
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};
use std::{path::Path, time::Duration};
use uuid::Uuid;
use window::ensure_window_allowed;

pub use actions::ComputerAction;
pub use display::{ComputerCapabilities, can_start, capabilities, capabilities_for};
pub use window::{WindowAllowlist, allowlist, set_allowlist};

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

fn require_active(session: &ComputerSession) -> AppResult<()> {
    match session.status.as_str() {
        "ACTIVE" => Ok(()),
        "PENDING" => Err(AppError::Conflict("sessão aguarda aprovação humana".into())),
        _ => Err(AppError::Conflict("sessão encerrada".into())),
    }
}
