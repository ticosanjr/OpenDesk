use crate::error::{AppError, AppResult};

use sqlx::SqlitePool;
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};
use tokio::sync::Mutex as AsyncMutex;

use super::RecipeIndex;
use super::sha256;
use super::versions::record_version;

/// Serialises reconciliation per workspace. Without it, a watcher sync that
/// scanned the folder before a pack installed recipes could prune the rows the
/// install had just written.
fn sync_lock(workspace_id: &str) -> Arc<AsyncMutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>> = OnceLock::new();
    let locks = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = locks
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .entry(workspace_id.to_string())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

const RECIPE_COLUMNS: &str = "id, workspace_id, name, file_path, content_sha256, trigger_type, \
    is_enabled, last_error, last_run_at, total_runs, updated_at";

pub async fn sync_workspace(
    pool: &SqlitePool,
    workspace_id: &str,
    root: &Path,
) -> AppResult<usize> {
    let lock = sync_lock(workspace_id);
    let _guard = lock.lock().await;
    sync_workspace_locked(pool, workspace_id, root).await
}

async fn sync_workspace_locked(
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
            match crate::parser::parse(&text) {
                Ok(recipe) => {
                    let id = recipe.id.clone();
                    let trigger = recipe.primary_trigger_type();
                    let hash = sha256(&text);
                    sqlx::query(
                        "INSERT INTO recipes_index (
                            id, workspace_id, name, file_path, content_sha256, trigger_type,
                            is_enabled, last_error
                         ) VALUES (?, ?, ?, ?, ?, ?, ?, NULL)
                         ON CONFLICT(workspace_id, id) DO UPDATE SET
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
            sqlx::query("DELETE FROM recipes_index WHERE workspace_id = ? AND id = ?")
                .bind(workspace_id)
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

pub async fn get(pool: &SqlitePool, workspace_id: &str, id: &str) -> AppResult<RecipeIndex> {
    let query =
        format!("SELECT {RECIPE_COLUMNS} FROM recipes_index WHERE workspace_id = ? AND id = ?");
    sqlx::query_as::<_, RecipeIndex>(&query)
        .bind(workspace_id)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("recipe {id}")))
}

pub async fn yaml_for(pool: &SqlitePool, workspace_id: &str, id: &str) -> AppResult<String> {
    let index = get(pool, workspace_id, id).await?;
    Ok(std::fs::read_to_string(index.file_path)?)
}

/// Toggle the top-level `enabled` flag without reformatting the rest of the
/// YAML. Comments on the flag line are preserved so the file remains the
/// source of truth and stays friendly to manual editing.
pub fn set_yaml_enabled(yaml: &str, enabled: bool) -> String {
    let mut output = String::new();
    let mut replaced = false;
    for line in yaml.lines() {
        let top_level = !line.starts_with(' ') && !line.starts_with('\t');
        if !replaced && top_level && line.trim_start().starts_with("enabled:") {
            let comment = line.find(" #").map(|index| &line[index..]).unwrap_or("");
            output.push_str(&format!(
                "enabled: {}{comment}\n",
                if enabled { "true" } else { "false" }
            ));
            replaced = true;
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    if !replaced {
        output.insert_str(
            0,
            if enabled {
                "enabled: true\n"
            } else {
                "enabled: false\n"
            },
        );
    }
    if !yaml.ends_with('\n') && output.ends_with('\n') {
        output.pop();
    }
    output
}

pub async fn set_enabled(
    pool: &SqlitePool,
    workspace_id: &str,
    id: &str,
    enabled: bool,
) -> AppResult<RecipeIndex> {
    let yaml = yaml_for(pool, workspace_id, id).await?;
    let updated = set_yaml_enabled(&yaml, enabled);
    save_yaml(pool, workspace_id, id, &updated).await
}

pub async fn save_yaml(
    pool: &SqlitePool,
    workspace_id: &str,
    id: &str,
    yaml: &str,
) -> AppResult<RecipeIndex> {
    let recipe = crate::parser::parse(yaml)?;
    if recipe.id != id {
        return Err(AppError::BadRequest(format!(
            "o id do YAML '{}' difere de '{}'",
            recipe.id, id
        )));
    }
    let root = opendesk_storage::repos::workspaces::get(pool, workspace_id)
        .await?
        .root_path;
    let path = Path::new(&root)
        .join(".opencode")
        .join("recipes")
        .join(format!("{id}.yaml"));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // New recipes always start disabled so triggers and routines cannot fire
    // before a human reviews the flow. Editing an existing file preserves the
    // current enabled flag; set_enabled is the explicit toggle.
    let content = if path.exists() {
        yaml.to_string()
    } else {
        set_yaml_enabled(yaml, false)
    };
    let lock = sync_lock(workspace_id);
    let _guard = lock.lock().await;
    let temp = path.with_extension(format!("yaml.{}.tmp", uuid::Uuid::now_v7()));
    std::fs::write(&temp, &content)?;
    std::fs::rename(&temp, &path)?;
    opendesk_engine::config_sync::watcher::expect_write(&path, Some(sha256(&content)));
    sync_workspace_locked(pool, workspace_id, Path::new(&root)).await?;
    record_version(pool, workspace_id, id, &content).await?;
    get(pool, workspace_id, id).await
}
