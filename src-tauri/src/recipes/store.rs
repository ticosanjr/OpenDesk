use crate::{
    error::{AppError, AppResult},
    recipes::parser::Recipe,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RecipeIndex {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub file_path: String,
    pub content_sha256: Option<String>,
    pub trigger_type: String,
    pub is_enabled: bool,
    pub last_error: Option<String>,
    pub last_run_at: Option<String>,
    pub total_runs: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RecipeRun {
    pub id: String,
    pub recipe_id: String,
    pub workspace_id: String,
    pub task_id: Option<String>,
    pub status: String,
    pub current_step_id: Option<String>,
    pub trigger_context_json: String,
    pub step_outputs_json: String,
    pub output_dir_path: String,
    pub dry_run: bool,
    pub error_message: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct HitlRequest {
    pub token: String,
    pub recipe_run_id: String,
    pub workspace_id: String,
    pub checkpoint_id: String,
    pub interaction_type: String,
    pub options_json: String,
    pub status: String,
    pub on_timeout: String,
    pub dispatched_channels_json: String,
    pub resolved_by_channel: Option<String>,
    pub resolved_by_sender: Option<String>,
    pub resolution_type: Option<String>,
    pub resolution_payload: Option<String>,
    pub expires_at: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

const RECIPE_COLUMNS: &str = "id, workspace_id, name, file_path, content_sha256, trigger_type, \
    is_enabled, last_error, last_run_at, total_runs, updated_at";
const RUN_COLUMNS: &str = "id, recipe_id, workspace_id, task_id, status, current_step_id, \
    trigger_context_json, step_outputs_json, output_dir_path, dry_run, error_message, \
    started_at, completed_at";
const HITL_COLUMNS: &str = "token, recipe_run_id, workspace_id, checkpoint_id, interaction_type, \
    options_json, status, on_timeout, dispatched_channels_json, resolved_by_channel, resolved_by_sender, \
    resolution_type, resolution_payload, expires_at, created_at, resolved_at";

fn sha256(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub async fn sync_workspace(
    pool: &SqlitePool,
    workspace_id: &str,
    root: &Path,
) -> AppResult<usize> {
    let dir = root.join(".opencode").join("recipes");
    std::fs::create_dir_all(&dir)?;
    let mut seen = Vec::new();
    let mut errors = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("yaml") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            match crate::recipes::parser::parse(&text) {
                Ok(recipe) => {
                    let id = recipe.id.clone();
                    let trigger = recipe.trigger.trigger_type.clone();
                    let hash = sha256(&text);
                    sqlx::query(
                        "INSERT INTO recipes_index (
                            id, workspace_id, name, file_path, content_sha256, trigger_type,
                            is_enabled, last_error
                         ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL)
                         ON CONFLICT(id) DO UPDATE SET
                            workspace_id = excluded.workspace_id,
                            name = excluded.name,
                            file_path = excluded.file_path,
                            content_sha256 = excluded.content_sha256,
                            trigger_type = excluded.trigger_type,
                            is_enabled = excluded.is_enabled,
                            last_error = NULL,
                            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
                    )
                    .bind(&id)
                    .bind(workspace_id)
                    .bind(&recipe.name)
                    .bind(path.to_string_lossy().to_string())
                    .bind(&hash)
                    .bind(&trigger)
                    .bind(recipe.enabled)
                    .execute(pool)
                    .await?;
                    seen.push(id);
                }
                Err(error) => errors.push(format!("{}: {}", path.display(), error)),
            }
        }
    }
    for error in errors {
        tracing::warn!(error, "recipe validation failed");
    }
    let indexes = list(pool, workspace_id).await?;
    for index in indexes {
        if !seen.contains(&index.id) {
            sqlx::query("DELETE FROM recipes_index WHERE id = ?")
                .bind(&index.id)
                .execute(pool)
                .await?;
        }
    }
    Ok(seen.len())
}

pub async fn list(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<RecipeIndex>> {
    let query =
        format!("SELECT {RECIPE_COLUMNS} FROM recipes_index WHERE workspace_id = ? ORDER BY name");
    Ok(sqlx::query_as::<_, RecipeIndex>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn list_all(pool: &SqlitePool) -> AppResult<Vec<RecipeIndex>> {
    let query = format!("SELECT {RECIPE_COLUMNS} FROM recipes_index ORDER BY workspace_id, name");
    Ok(sqlx::query_as::<_, RecipeIndex>(&query)
        .fetch_all(pool)
        .await?)
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<RecipeIndex> {
    let query = format!("SELECT {RECIPE_COLUMNS} FROM recipes_index WHERE id = ?");
    sqlx::query_as::<_, RecipeIndex>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("recipe {id}")))
}

pub async fn yaml_for(pool: &SqlitePool, id: &str) -> AppResult<String> {
    let index = get(pool, id).await?;
    Ok(std::fs::read_to_string(index.file_path)?)
}

pub async fn save_yaml(
    pool: &SqlitePool,
    workspace_id: &str,
    id: &str,
    yaml: &str,
) -> AppResult<RecipeIndex> {
    let recipe = crate::recipes::parser::parse(yaml)?;
    if recipe.id != id {
        return Err(AppError::BadRequest(format!(
            "o id do YAML '{}' difere de '{}'",
            recipe.id, id
        )));
    }
    let root = crate::db::workspaces::get(pool, workspace_id)
        .await?
        .root_path;
    let path = Path::new(&root)
        .join(".opencode")
        .join("recipes")
        .join(format!("{id}.yaml"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, yaml)?;
    crate::config_sync::watcher::expect_write(&path, Some(sha256(yaml)));
    sync_workspace(pool, workspace_id, Path::new(&root)).await?;
    get(pool, id).await
}

pub async fn create_run(
    pool: &SqlitePool,
    recipe: &Recipe,
    workspace_id: &str,
    task_id: Option<&str>,
    trigger_context: &serde_json::Value,
    output_dir: &str,
    dry_run: bool,
) -> AppResult<RecipeRun> {
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO recipe_runs (
            id, recipe_id, workspace_id, task_id, status, trigger_context_json,
            output_dir_path, dry_run
         ) VALUES (?, ?, ?, ?, 'RUNNING_STEP', ?, ?, ?)",
    )
    .bind(&id)
    .bind(&recipe.id)
    .bind(workspace_id)
    .bind(task_id)
    .bind(trigger_context.to_string())
    .bind(output_dir)
    .bind(dry_run)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE recipes_index SET total_runs = total_runs + 1,
            last_run_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(&recipe.id)
    .execute(pool)
    .await?;
    get_run(pool, &id).await
}

pub async fn get_run(pool: &SqlitePool, id: &str) -> AppResult<RecipeRun> {
    let query = format!("SELECT {RUN_COLUMNS} FROM recipe_runs WHERE id = ?");
    sqlx::query_as::<_, RecipeRun>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("recipe run {id}")))
}

pub async fn list_runs(
    pool: &SqlitePool,
    workspace_id: &str,
    limit: i64,
) -> AppResult<Vec<RecipeRun>> {
    let query = format!(
        "SELECT {RUN_COLUMNS} FROM recipe_runs WHERE workspace_id = ?
         ORDER BY started_at DESC LIMIT ?"
    );
    Ok(sqlx::query_as::<_, RecipeRun>(&query)
        .bind(workspace_id)
        .bind(limit.clamp(1, 500))
        .fetch_all(pool)
        .await?)
}

pub async fn update_run(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    step_id: Option<&str>,
    outputs: Option<&serde_json::Value>,
    error: Option<&str>,
    completed: bool,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE recipe_runs SET status = ?, current_step_id = COALESCE(?, current_step_id),
            step_outputs_json = COALESCE(?, step_outputs_json), error_message = ?,
            completed_at = CASE WHEN ? THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') ELSE completed_at END
         WHERE id = ?",
    )
    .bind(status)
    .bind(step_id)
    .bind(outputs.map(|value| value.to_string()))
    .bind(error)
    .bind(completed)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn create_hitl(
    pool: &SqlitePool,
    token: &str,
    run: &RecipeRun,
    checkpoint_id: &str,
    interaction_type: &str,
    options: &[String],
    on_timeout: &str,
    expires_at: &str,
) -> AppResult<HitlRequest> {
    sqlx::query(
        "INSERT INTO hitl_requests (
            token, recipe_run_id, workspace_id, checkpoint_id, interaction_type,
            options_json, on_timeout, expires_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(token)
    .bind(&run.id)
    .bind(&run.workspace_id)
    .bind(checkpoint_id)
    .bind(interaction_type)
    .bind(serde_json::to_string(options).unwrap_or_else(|_| "[]".into()))
    .bind(on_timeout)
    .bind(expires_at)
    .execute(pool)
    .await?;
    get_hitl(pool, token).await
}

pub async fn get_hitl(pool: &SqlitePool, token: &str) -> AppResult<HitlRequest> {
    let query = format!("SELECT {HITL_COLUMNS} FROM hitl_requests WHERE token = ?");
    sqlx::query_as::<_, HitlRequest>(&query)
        .bind(token)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("hitl {token}")))
}

pub async fn resolve_hitl(
    pool: &SqlitePool,
    token: &str,
    channel: &str,
    sender: Option<&str>,
    resolution_type: &str,
    payload: Option<&str>,
) -> AppResult<Option<HitlRequest>> {
    let result = sqlx::query(
        "UPDATE hitl_requests SET status = 'RESOLVED', resolved_by_channel = ?,
            resolved_by_sender = ?, resolution_type = ?, resolution_payload = ?,
            resolved_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE token = ? AND status = 'PENDING'",
    )
    .bind(channel)
    .bind(sender)
    .bind(resolution_type)
    .bind(payload)
    .bind(token)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Ok(None);
    }
    Ok(Some(get_hitl(pool, token).await?))
}

pub async fn dispatch_channel(
    pool: &SqlitePool,
    token: &str,
    channel: &str,
    external_id: &str,
) -> AppResult<()> {
    let current = get_hitl(pool, token).await?;
    let mut value: serde_json::Value = serde_json::from_str(&current.dispatched_channels_json)
        .unwrap_or_else(|_| serde_json::json!({}));
    value[channel] = serde_json::Value::String(external_id.to_string());
    sqlx::query("UPDATE hitl_requests SET dispatched_channels_json = ? WHERE token = ?")
        .bind(value.to_string())
        .bind(token)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn pending_hitl(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<HitlRequest>> {
    let query = format!(
        "SELECT {HITL_COLUMNS} FROM hitl_requests
         WHERE workspace_id = ? AND status = 'PENDING' ORDER BY created_at"
    );
    Ok(sqlx::query_as::<_, HitlRequest>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn pending_hitl_for_run(
    pool: &SqlitePool,
    recipe_run_id: &str,
) -> AppResult<Vec<HitlRequest>> {
    let query = format!(
        "SELECT {HITL_COLUMNS} FROM hitl_requests
         WHERE recipe_run_id = ? AND status = 'PENDING' ORDER BY created_at"
    );
    Ok(sqlx::query_as::<_, HitlRequest>(&query)
        .bind(recipe_run_id)
        .fetch_all(pool)
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn indexes_saves_and_runs_recipe() {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("r.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("ws");
        let yaml = "id: teste\nname: Teste\ntrigger:\n  type: MANUAL\n  config: {}\nsteps:\n  - id: s1\n    name: Um\n    prompt: oi\n";
        let index = save_yaml(&pool, "ws-1", "teste", yaml).await.expect("save");
        assert_eq!(index.id, "teste");
        let recipe = crate::recipes::parser::parse(yaml).expect("parse");
        let run = create_run(
            &pool,
            &recipe,
            "ws-1",
            None,
            &serde_json::json!({}),
            "/tmp/out",
            false,
        )
        .await
        .expect("run");
        assert_eq!(run.status, "RUNNING_STEP");
        assert_eq!(list(&pool, "ws-1").await.expect("list").len(), 1);

        let hitl = create_hitl(
            &pool,
            "8F2A",
            &run,
            "h1",
            "BINARY_GATE",
            &[],
            "PAUSE_AND_ALERT",
            "2999-01-01T00:00:00Z",
        )
        .await
        .expect("hitl");
        assert_eq!(hitl.status, "PENDING");
        dispatch_channel(&pool, "8F2A", "SIGNAL", "ts-1")
            .await
            .expect("dispatch");
        assert_eq!(pending_hitl(&pool, "ws-1").await.expect("pending").len(), 1);
        let resolved = resolve_hitl(&pool, "8F2A", "SIGNAL", Some("+1"), "APPROVED", None)
            .await
            .expect("resolve")
            .expect("resolved");
        assert_eq!(resolved.status, "RESOLVED");
        assert!(
            resolve_hitl(&pool, "8F2A", "EMAIL", None, "REJECTED", None)
                .await
                .expect("second")
                .is_none()
        );
    }
}
