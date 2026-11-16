//! Computer-use bridge.
//!
//! This is deliberately a thin, auditable executor over host tools
//! (`grim`/`import` for screenshots; `xdotool`/`ydotool` for input). It never
//! embeds a screen stream or bypasses the OS: on Wayland input depends on
//! `ydotool` having its daemon running. Every action is counted and summarized
//! without storing typed text.

use crate::{
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
    pub created_at: String,
    pub ended_at: Option<String>,
}

const COLUMNS: &str = "id, workspace_id, run_id, status, require_approval, actions, \
    last_action_json, screenshot_path, created_at, ended_at";

pub async fn create(
    pool: &SqlitePool,
    workspace_id: &str,
    run_id: Option<&str>,
    require_approval: bool,
) -> AppResult<ComputerSession> {
    let id = Uuid::now_v7().to_string();
    let status = if require_approval {
        "PENDING"
    } else {
        "ACTIVE"
    };
    sqlx::query(
        "INSERT INTO computer_sessions (id, workspace_id, run_id, status, require_approval)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(run_id)
    .bind(status)
    .bind(require_approval)
    .execute(pool)
    .await?;
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
    let capabilities = capabilities();
    let Some(tool) = capabilities.screenshot_tool.as_deref() else {
        return Err(AppError::Conflict("nenhuma ferramenta de captura".into()));
    };
    let dir = workspace_root
        .join("output")
        .join(chrono::Utc::now().format("%Y-%m").to_string())
        .join("computer");
    tokio::fs::create_dir_all(&dir).await?;
    let file = dir.join(format!("{id}.png"));
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        tokio::process::Command::new(tool).arg(&file).output(),
    )
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
    let capabilities = capabilities();
    let Some(tool) = capabilities.input_tool.clone() else {
        return Err(AppError::Conflict("nenhuma ferramenta de entrada".into()));
    };
    let (program, args, summary) = build_action(&capabilities, &tool, &action)?;
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        tokio::process::Command::new(&program).args(&args).output(),
    )
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

    #[tokio::test]
    async fn stores_sessions_and_requires_approval() {
        let ctx = crate::test_support::context().await;
        let session = create(&ctx.state.db, &ctx.workspace_id, None, true)
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
