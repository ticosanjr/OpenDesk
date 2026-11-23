use crate::{
    app_state::AppState,
    db::{tasks, workspaces},
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct MemoryBody {
    pub content: String,
    #[serde(default)]
    pub run_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct MemoryQuery {
    pub run_id: Option<String>,
}

fn memory_marker(task_id: &str, run_id: Option<&str>) -> String {
    format!(
        "<!-- opendesk:memory task:{task_id} run:{} -->",
        run_id.unwrap_or("-")
    )
}

fn memory_block(marker: &str, content: &str) -> String {
    format!(
        "\n{marker}\n- [{}] {}\n<!-- /opendesk:memory -->\n",
        chrono::Utc::now().format("%Y-%m-%d"),
        content.replace('\n', " ")
    )
}

fn remove_memory_blocks(content: &str, task_id: &str, run_id: Option<&str>) -> String {
    let prefix = format!("<!-- opendesk:memory task:{task_id}");
    let wanted = run_id.map(|run_id| memory_marker(task_id, Some(run_id)));
    let mut output = String::new();
    let mut skipping = false;
    for line in content.lines() {
        if line.starts_with(&prefix) {
            let matches = wanted.as_deref().is_none_or(|marker| line.trim() == marker);
            skipping = matches;
            if !matches {
                output.push_str(line);
                output.push('\n');
            }
            continue;
        }
        if skipping {
            if line.trim() == "<!-- /opendesk:memory -->" {
                skipping = false;
            }
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output.trim_end().to_string() + if output.trim().is_empty() { "" } else { "\n" }
}

pub async fn get_task_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let task = tasks::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &task.workspace_id).await?;
    let path = std::path::Path::new(&workspace.root_path)
        .join(".opencode")
        .join("MEMORY.md");
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    Ok(Json(serde_json::json!({ "content": content })))
}

pub async fn post_task_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<MemoryBody>,
) -> AppResult<Json<serde_json::Value>> {
    let content = body.content.trim();
    if content.is_empty() {
        return Err(AppError::BadRequest("memory content is required".into()));
    }
    let task = tasks::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &task.workspace_id).await?;
    let dir = std::path::Path::new(&workspace.root_path).join(".opencode");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("MEMORY.md");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let cleaned = remove_memory_blocks(&existing, &id, body.run_id.as_deref());
    let marker = memory_marker(&id, body.run_id.as_deref());
    let next = format!("{}{}", cleaned.trim_end(), memory_block(&marker, content));
    crate::config_sync::file_ops::write_text_atomic(&path, &next, None, &state.paths.backups_dir)?;
    Ok(Json(serde_json::json!({ "ok": true, "path": path })))
}

pub async fn delete_task_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<MemoryQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let task = tasks::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &task.workspace_id).await?;
    let path = std::path::Path::new(&workspace.root_path)
        .join(".opencode")
        .join("MEMORY.md");
    if !path.exists() {
        return Ok(Json(serde_json::json!({ "ok": true, "removed": false })));
    }
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let next = remove_memory_blocks(&existing, &id, query.run_id.as_deref());
    if next != existing {
        crate::config_sync::file_ops::write_text_atomic(
            &path,
            &next,
            None,
            &state.paths.backups_dir,
        )?;
    }
    Ok(Json(
        serde_json::json!({ "ok": true, "removed": next != existing }),
    ))
}
