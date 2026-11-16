use crate::{
    error::{AppError, AppResult},
    recipes::parser::Recipe,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, SqlitePool};
use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, OnceLock},
};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

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
    #[sqlx(default)]
    pub cancel_requested: bool,
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
    pub title: Option<String>,
    pub question_markdown: Option<String>,
    pub context_json: String,
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
    trigger_context_json, step_outputs_json, output_dir_path, dry_run, cancel_requested, \
    error_message, started_at, completed_at";
const HITL_COLUMNS: &str = "token, recipe_run_id, workspace_id, checkpoint_id, interaction_type, \
    title, question_markdown, context_json, options_json, status, on_timeout, \
    dispatched_channels_json, resolved_by_channel, resolved_by_sender, resolution_type, \
    resolution_payload, expires_at, created_at, resolved_at";

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
    let lock = sync_lock(workspace_id);
    let _guard = lock.lock().await;
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
    // New recipes always start disabled so triggers and routines cannot fire
    // before a human reviews the flow. Editing an existing file preserves the
    // current enabled flag; set_enabled is the explicit toggle.
    let content = if path.exists() {
        yaml.to_string()
    } else {
        set_yaml_enabled(yaml, false)
    };
    std::fs::write(&path, &content)?;
    crate::config_sync::watcher::expect_write(&path, Some(sha256(&content)));
    sync_workspace(pool, workspace_id, Path::new(&root)).await?;
    get(pool, workspace_id, id).await
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
            last_run_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE workspace_id = ? AND id = ?",
    )
    .bind(workspace_id)
    .bind(&recipe.id)
    .execute(pool)
    .await?;
    get_run(pool, &id).await
}

pub async fn latest_run_for_task(pool: &SqlitePool, task_id: &str) -> AppResult<Option<RecipeRun>> {
    let query = format!(
        "SELECT {RUN_COLUMNS} FROM recipe_runs WHERE task_id = ? ORDER BY started_at DESC LIMIT 1"
    );
    Ok(sqlx::query_as::<_, RecipeRun>(&query)
        .bind(task_id)
        .fetch_optional(pool)
        .await?)
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
    title: &str,
    question: &str,
    context: &serde_json::Value,
    options: &[String],
    on_timeout: &str,
    expires_at: &str,
) -> AppResult<HitlRequest> {
    sqlx::query(
        "INSERT INTO hitl_requests (
            token, recipe_run_id, workspace_id, checkpoint_id, interaction_type,
            title, question_markdown, context_json, options_json, on_timeout, expires_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(token)
    .bind(&run.id)
    .bind(&run.workspace_id)
    .bind(checkpoint_id)
    .bind(interaction_type)
    .bind(title)
    .bind(question)
    .bind(context.to_string())
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

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RecipeRunNode {
    pub id: String,
    pub run_id: String,
    pub node_index: i64,
    #[sqlx(default)]
    pub node_path: Option<String>,
    #[sqlx(default)]
    pub parent_path: Option<String>,
    pub node_id: String,
    pub kind: String,
    #[sqlx(default)]
    pub item_key: Option<String>,
    #[sqlx(default)]
    pub join_policy: Option<String>,
    pub status: String,
    pub attempts: i64,
    pub session_id: Option<String>,
    pub output_json: Option<String>,
    pub error_message: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

const NODE_COLUMNS: &str = "id, run_id, node_index, node_path, parent_path, node_id, kind, \
    item_key, join_policy, status, attempts, session_id, output_json, error_message, \
    started_at, finished_at";

pub async fn sync_run_nodes(
    pool: &SqlitePool,
    run_id: &str,
    flow: &[crate::recipes::parser::FlowItem],
) -> AppResult<()> {
    let mut seeds = Vec::new();
    collect_node_seeds(flow, None, &mut seeds);
    for (index, seed) in seeds.iter().enumerate() {
        let id = format!("{run_id}:{}", seed.path);
        sqlx::query(
            "INSERT INTO recipe_run_nodes
                (id, run_id, node_index, node_path, parent_path, node_id, kind,
                 item_key, join_policy)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(run_id, node_index) DO NOTHING",
        )
        .bind(id)
        .bind(run_id)
        .bind(index as i64)
        .bind(&seed.path)
        .bind(&seed.parent)
        .bind(&seed.node_id)
        .bind(seed.kind)
        .bind(&seed.item_key)
        .bind(&seed.join_policy)
        .execute(pool)
        .await?;
    }
    Ok(())
}

struct NodeSeed {
    path: String,
    parent: Option<String>,
    node_id: String,
    kind: &'static str,
    item_key: Option<String>,
    join_policy: Option<String>,
}

fn join_path(parent: Option<&str>, id: &str) -> String {
    match parent {
        Some(parent) if !parent.is_empty() => format!("{parent}/{id}"),
        _ => id.to_string(),
    }
}

fn collect_node_seeds(
    flow: &[crate::recipes::parser::FlowItem],
    parent: Option<&str>,
    seeds: &mut Vec<NodeSeed>,
) {
    use crate::recipes::parser::FlowItem;
    for (index, item) in flow.iter().enumerate() {
        let (node_id, kind) = crate::recipes::parser::node_descriptor(item, index);
        let path = join_path(parent, &node_id);
        match item {
            FlowItem::Step(_) | FlowItem::Hitl(_) | FlowItem::Sink(_) => {
                seeds.push(NodeSeed {
                    path,
                    parent: parent.map(str::to_string),
                    node_id,
                    kind,
                    item_key: None,
                    join_policy: None,
                });
            }
            FlowItem::Branch(branch) => {
                seeds.push(NodeSeed {
                    path: path.clone(),
                    parent: parent.map(str::to_string),
                    node_id,
                    kind,
                    item_key: None,
                    join_policy: None,
                });
                collect_node_seeds(&branch.then, Some(&path), seeds);
                collect_node_seeds(&branch.otherwise, Some(&path), seeds);
            }
            FlowItem::Parallel(parallel) => {
                seeds.push(NodeSeed {
                    path: path.clone(),
                    parent: parent.map(str::to_string),
                    node_id,
                    kind,
                    item_key: None,
                    join_policy: Some(parallel.join.clone()),
                });
                for (branch_index, branch) in parallel.branches.iter().enumerate() {
                    let branch_path = join_path(Some(&path), &format!("b{branch_index}"));
                    collect_node_seeds(branch, Some(&branch_path), seeds);
                }
            }
            FlowItem::Map(_) => {
                seeds.push(NodeSeed {
                    path,
                    parent: parent.map(str::to_string),
                    node_id,
                    kind,
                    item_key: None,
                    join_policy: None,
                });
                // Map children are materialized per item at runtime.
            }
        }
    }
}

pub async fn list_run_nodes(pool: &SqlitePool, run_id: &str) -> AppResult<Vec<RecipeRunNode>> {
    let query =
        format!("SELECT {NODE_COLUMNS} FROM recipe_run_nodes WHERE run_id = ? ORDER BY node_index");
    Ok(sqlx::query_as::<_, RecipeRunNode>(&query)
        .bind(run_id)
        .fetch_all(pool)
        .await?)
}

/// All durable nodes below a group path (`prefix/...`). The range uses the
/// ASCII order between `/` and `0`, avoiding LIKE wildcards in user ids.
pub async fn list_nodes_under(
    pool: &SqlitePool,
    run_id: &str,
    prefix: &str,
) -> AppResult<Vec<RecipeRunNode>> {
    let lower = format!("{prefix}/");
    let upper = format!("{prefix}0");
    let query = format!(
        "SELECT {NODE_COLUMNS} FROM recipe_run_nodes
         WHERE run_id = ? AND node_path > ? AND node_path < ?
         ORDER BY node_index"
    );
    Ok(sqlx::query_as::<_, RecipeRunNode>(&query)
        .bind(run_id)
        .bind(lower)
        .bind(upper)
        .fetch_all(pool)
        .await?)
}

pub async fn mark_node_running(
    pool: &SqlitePool,
    run_id: &str,
    node_index: usize,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE recipe_run_nodes
         SET status = 'RUNNING', attempts = attempts + 1,
             started_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
             finished_at = NULL, error_message = NULL
         WHERE run_id = ? AND node_index = ?",
    )
    .bind(run_id)
    .bind(node_index as i64)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_node_waiting(
    pool: &SqlitePool,
    run_id: &str,
    node_index: usize,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE recipe_run_nodes SET status = 'WAITING_HITL'
         WHERE run_id = ? AND node_index = ?",
    )
    .bind(run_id)
    .bind(node_index as i64)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_node_done(
    pool: &SqlitePool,
    run_id: &str,
    node_index: usize,
    output: Option<&serde_json::Value>,
    session_id: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE recipe_run_nodes
         SET status = 'DONE', output_json = COALESCE(?, output_json),
             session_id = COALESCE(?, session_id),
             finished_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE run_id = ? AND node_index = ?",
    )
    .bind(output.map(|value| value.to_string()))
    .bind(session_id)
    .bind(run_id)
    .bind(node_index as i64)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn mark_node_skipped(
    pool: &SqlitePool,
    run_id: &str,
    node_index: usize,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE recipe_run_nodes
         SET status = 'SKIPPED', finished_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE run_id = ? AND node_index = ?",
    )
    .bind(run_id)
    .bind(node_index as i64)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn reset_node(pool: &SqlitePool, run_id: &str, node_index: usize) -> AppResult<()> {
    sqlx::query(
        "UPDATE recipe_run_nodes
         SET status = 'PENDING', error_message = NULL, finished_at = NULL
         WHERE run_id = ? AND node_index = ?",
    )
    .bind(run_id)
    .bind(node_index as i64)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fail_running_nodes(pool: &SqlitePool, run_id: &str, error: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE recipe_run_nodes
         SET status = 'FAILED', error_message = ?,
             finished_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE run_id = ? AND status IN ('RUNNING', 'WAITING_HITL')",
    )
    .bind(error)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Materializes a dynamic DAG node (map item groups) with a unique path.
#[allow(clippy::too_many_arguments)]
pub async fn ensure_dag_node(
    pool: &SqlitePool,
    run_id: &str,
    path: &str,
    parent_path: Option<&str>,
    node_id: &str,
    kind: &str,
    item_key: Option<&str>,
    join_policy: Option<&str>,
) -> AppResult<()> {
    let id = format!("{run_id}:{path}");
    sqlx::query(
        "INSERT INTO recipe_run_nodes
            (id, run_id, node_index, node_path, parent_path, node_id, kind,
             item_key, join_policy)
         SELECT ?, ?, COALESCE(MAX(node_index), -1) + 1, ?, ?, ?, ?, ?, ?
         FROM recipe_run_nodes WHERE run_id = ?
         ON CONFLICT(run_id, node_path) DO NOTHING",
    )
    .bind(id)
    .bind(run_id)
    .bind(path)
    .bind(parent_path)
    .bind(node_id)
    .bind(kind)
    .bind(item_key)
    .bind(join_policy)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn node_by_path(
    pool: &SqlitePool,
    run_id: &str,
    path: &str,
) -> AppResult<Option<RecipeRunNode>> {
    let query =
        format!("SELECT {NODE_COLUMNS} FROM recipe_run_nodes WHERE run_id = ? AND node_path = ?");
    Ok(sqlx::query_as::<_, RecipeRunNode>(&query)
        .bind(run_id)
        .bind(path)
        .fetch_optional(pool)
        .await?)
}

pub async fn mark_node_path(
    pool: &SqlitePool,
    run_id: &str,
    path: &str,
    status: &str,
    output: Option<&serde_json::Value>,
    session_id: Option<&str>,
    error: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE recipe_run_nodes
         SET status = ?, output_json = COALESCE(?, output_json),
             session_id = COALESCE(?, session_id),
             error_message = ?,
             started_at = CASE WHEN ? = 'RUNNING'
                 THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') ELSE started_at END,
             finished_at = CASE WHEN ? IN ('DONE','FAILED','SKIPPED')
                 THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') ELSE NULL END,
             attempts = attempts + ?
         WHERE run_id = ? AND node_path = ?",
    )
    .bind(status)
    .bind(output.map(|value| value.to_string()))
    .bind(session_id)
    .bind(error)
    .bind(status)
    .bind(status)
    .bind(if status == "RUNNING" { 1_i64 } else { 0 })
    .bind(run_id)
    .bind(path)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn reset_node_path(pool: &SqlitePool, run_id: &str, path: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE recipe_run_nodes
         SET status = 'PENDING', error_message = NULL, finished_at = NULL
         WHERE run_id = ? AND node_path = ?",
    )
    .bind(run_id)
    .bind(path)
    .execute(pool)
    .await?;
    Ok(())
}

/// Marks nodes that were statically planned but never selected (branch side
/// not taken, map template) as SKIPPED before completing the run.
pub async fn mark_pending_skipped(pool: &SqlitePool, run_id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE recipe_run_nodes
         SET status = 'SKIPPED', finished_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE run_id = ? AND status = 'PENDING'",
    )
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn resumable_runs(pool: &SqlitePool) -> AppResult<Vec<RecipeRun>> {
    let query = format!(
        "SELECT {RUN_COLUMNS} FROM recipe_runs
         WHERE status IN ('RUNNING_STEP', 'RUNNING_SINKS', 'WAITING_HITL')
           AND cancel_requested = 0 AND dry_run = 0
         ORDER BY started_at"
    );
    Ok(sqlx::query_as::<_, RecipeRun>(&query)
        .fetch_all(pool)
        .await?)
}

pub async fn is_cancel_requested(pool: &SqlitePool, run_id: &str) -> AppResult<bool> {
    Ok(
        sqlx::query_scalar::<_, bool>("SELECT cancel_requested FROM recipe_runs WHERE id = ?")
            .bind(run_id)
            .fetch_optional(pool)
            .await?
            .unwrap_or(false),
    )
}

pub async fn request_cancel(pool: &SqlitePool, run_id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE recipe_runs SET cancel_requested = 1
         WHERE id = ? AND status IN ('RUNNING_STEP', 'RUNNING_SINKS', 'WAITING_HITL')",
    )
    .bind(run_id)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE hitl_requests SET status = 'CANCELLED'
         WHERE recipe_run_id = ? AND status = 'PENDING'",
    )
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn reset_for_retry(pool: &SqlitePool, run_id: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE recipe_runs SET status = 'RUNNING_STEP', cancel_requested = 0,
                error_message = NULL, completed_at = NULL, current_step_id = NULL
         WHERE id = ?",
    )
    .bind(run_id)
    .execute(pool)
    .await?;
    sqlx::query(
        "UPDATE recipe_run_nodes
         SET status = 'PENDING', error_message = NULL, finished_at = NULL
         WHERE run_id = ? AND status IN ('FAILED', 'RUNNING', 'WAITING_HITL')",
    )
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn pending_hitl_for_checkpoint(
    pool: &SqlitePool,
    run_id: &str,
    checkpoint_id: &str,
) -> AppResult<Option<HitlRequest>> {
    let query = format!(
        "SELECT {HITL_COLUMNS} FROM hitl_requests
         WHERE recipe_run_id = ? AND checkpoint_id = ? AND status = 'PENDING'
         ORDER BY created_at DESC LIMIT 1"
    );
    Ok(sqlx::query_as::<_, HitlRequest>(&query)
        .bind(run_id)
        .bind(checkpoint_id)
        .fetch_optional(pool)
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
            "Receita teste",
            "Aprovar o checkpoint?",
            &serde_json::json!({ "recipe_name": "Receita teste" }),
            &[],
            "PAUSE_AND_ALERT",
            "2999-01-01T00:00:00Z",
        )
        .await
        .expect("hitl");
        assert_eq!(hitl.status, "PENDING");
        assert_eq!(hitl.title.as_deref(), Some("Receita teste"));
        assert_eq!(
            hitl.question_markdown.as_deref(),
            Some("Aprovar o checkpoint?")
        );
        assert!(
            hitl.context_json.contains("Receita teste"),
            "context: {}",
            hitl.context_json
        );
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

    #[tokio::test]
    async fn same_recipe_id_is_scoped_per_workspace() {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("r.db"))
            .await
            .expect("db");
        for workspace in ["ws-1", "ws-2"] {
            let root = temp.path().join(workspace);
            std::fs::create_dir_all(root.join(".opencode/recipes")).expect("dir");
            sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES (?, ?, ?)")
                .bind(workspace)
                .bind(workspace)
                .bind(root.to_string_lossy().to_string())
                .execute(&pool)
                .await
                .expect("ws");
            std::fs::write(
                root.join(".opencode/recipes/shared.yaml"),
                "id: shared\nname: Compartilhada\ntrigger:\n  type: MANUAL\n  config: {}\nsteps:\n  - id: s1\n    name: Um\n    prompt: oi\n",
            )
            .expect("yaml");
            sync_workspace(&pool, workspace, &root).await.expect("sync");
        }

        let first = list(&pool, "ws-1").await.expect("list 1");
        let second = list(&pool, "ws-2").await.expect("list 2");
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        assert_ne!(first[0].file_path, second[0].file_path);
        assert!(first[0].file_path.contains("ws-1"));
        assert!(
            get(&pool, "ws-1", "shared").await.is_ok(),
            "each workspace keeps its own shared id"
        );
    }

    #[tokio::test]
    async fn concurrent_reconciliation_keeps_all_recipes() {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("r.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("ws");
        let dir = temp.path().join(".opencode/recipes");
        std::fs::create_dir_all(&dir).expect("dir");
        for id in ["um", "dois", "tres"] {
            std::fs::write(
                dir.join(format!("{id}.yaml")),
                format!(
                    "id: {id}\nname: {id}\ntrigger:\n  type: MANUAL\n  config: {{}}\nsteps:\n  - id: s1\n    name: Um\n    prompt: oi\n"
                ),
            )
            .expect("yaml");
        }
        let mut tasks = Vec::new();
        for _ in 0..8 {
            let pool = pool.clone();
            let root = temp.path().to_path_buf();
            tasks.push(tokio::spawn(async move {
                sync_workspace(&pool, "ws-1", &root).await
            }));
        }
        for task in tasks {
            task.await.expect("join").expect("sync");
        }
        assert_eq!(list(&pool, "ws-1").await.expect("list").len(), 3);
    }

    #[tokio::test]
    async fn new_recipes_start_disabled_and_edits_preserve_the_flag() {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("r.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("ws");
        let yaml = "id: nova\nname: Nova\nenabled: true\ntriggers: []\nflow:\n  - type: step\n    id: s1\n    name: Um\n    prompt: oi\n";
        let index = save_yaml(&pool, "ws-1", "nova", yaml).await.expect("save");
        assert!(!index.is_enabled, "new recipes must start disabled");
        let stored = yaml_for(&pool, "ws-1", "nova").await.expect("yaml");
        assert!(stored.contains("enabled: false"), "{stored}");

        let enabled = set_enabled(&pool, "ws-1", "nova", true)
            .await
            .expect("enable");
        assert!(enabled.is_enabled);

        let edited = yaml.replace("Nova", "Nova editada");
        let saved = save_yaml(&pool, "ws-1", "nova", &edited)
            .await
            .expect("edit");
        assert!(saved.is_enabled, "editing must preserve the enabled flag");
    }

    #[tokio::test]
    async fn persists_nodes_and_cancel_flag() {
        let ctx = crate::test_support::context().await;
        let yaml = "id: durable\nname: Durable\ntrigger:\n  type: MANUAL\n  config: {}\nflow:\n  - type: step\n    id: s1\n    name: Passo 1\n    prompt: oi\n  - type: hitl\n    id: h1\n    interaction_type: BINARY_GATE\n    prompt_template: ok?\n  - type: sink\n    sink_type: DOC_COMPILER\n    config: {}\n";
        let recipe = crate::recipes::parser::parse(yaml).expect("recipe");
        save_yaml(&ctx.state.db, &ctx.workspace_id, "durable", yaml)
            .await
            .expect("index");
        let run = create_run(
            &ctx.state.db,
            &recipe,
            &ctx.workspace_id,
            None,
            &serde_json::json!({}),
            "/tmp/out",
            false,
        )
        .await
        .expect("run");
        sync_run_nodes(&ctx.state.db, &run.id, &recipe.flow)
            .await
            .expect("nodes");
        let nodes = list_run_nodes(&ctx.state.db, &run.id).await.expect("list");
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].kind, "step");
        assert_eq!(nodes[1].kind, "hitl");
        assert_eq!(nodes[2].kind, "sink");

        mark_node_running(&ctx.state.db, &run.id, 0)
            .await
            .expect("running");
        mark_node_done(
            &ctx.state.db,
            &run.id,
            0,
            Some(&serde_json::json!({"ok": true})),
            Some("s"),
        )
        .await
        .expect("done");
        let nodes = list_run_nodes(&ctx.state.db, &run.id).await.expect("list");
        assert_eq!(nodes[0].status, "DONE");
        assert_eq!(nodes[0].attempts, 1);

        assert_eq!(
            resumable_runs(&ctx.state.db).await.expect("resume").len(),
            1
        );
        request_cancel(&ctx.state.db, &run.id)
            .await
            .expect("cancel");
        assert!(
            is_cancel_requested(&ctx.state.db, &run.id)
                .await
                .expect("flag")
        );
        assert!(
            resumable_runs(&ctx.state.db)
                .await
                .expect("resume")
                .is_empty()
        );

        reset_for_retry(&ctx.state.db, &run.id)
            .await
            .expect("retry");
        assert!(
            !is_cancel_requested(&ctx.state.db, &run.id)
                .await
                .expect("flag")
        );
        let nodes = list_run_nodes(&ctx.state.db, &run.id).await.expect("list");
        assert_eq!(nodes[0].status, "DONE");
        assert_eq!(nodes[1].status, "PENDING");
    }

    #[test]
    fn toggles_enabled_preserving_comment_and_final_newline() {
        let yaml = "id: qa\nenabled: true # liga/desliga\nflow: []\n";
        let output = set_yaml_enabled(yaml, false);
        assert!(output.contains("enabled: false # liga/desliga"));
        assert!(output.ends_with('\n'));
        assert!(output.contains("flow: []"));
    }

    #[test]
    fn adds_enabled_when_missing_and_keeps_missing_final_newline() {
        let yaml = "id: qa\nflow: []";
        let output = set_yaml_enabled(yaml, true);
        assert!(output.starts_with("enabled: true\n"));
        assert!(!output.ends_with('\n'));
    }

    #[tokio::test]
    async fn seeds_path_addressed_nodes_for_dag_groups() {
        let ctx = crate::test_support::context().await;
        let yaml = r#"
id: dag-paths
name: DAG paths
flow:
  - type: branch
    id: decidir
    when: "true"
    then:
      - type: step
        id: t1
        name: T1
        prompt: a
    else:
      - type: step
        id: e1
        name: E1
        prompt: b
  - type: parallel
    id: lote
    branches:
      - - type: step
          id: p1
          name: P1
          prompt: a
      - - type: step
          id: p2
          name: P2
          prompt: b
  - type: map
    id: arquivos
    items: "[]"
    flow:
      - type: step
        id: analisar
        name: Analisar
        prompt: c
"#;
        save_yaml(&ctx.state.db, &ctx.workspace_id, "dag-paths", yaml)
            .await
            .expect("save");
        let recipe = crate::recipes::parser::parse(yaml).expect("recipe");
        let run = create_run(
            &ctx.state.db,
            &recipe,
            &ctx.workspace_id,
            None,
            &serde_json::json!({}),
            "/tmp/out",
            false,
        )
        .await
        .expect("run");
        sync_run_nodes(&ctx.state.db, &run.id, &recipe.flow)
            .await
            .expect("sync");
        let nodes = list_run_nodes(&ctx.state.db, &run.id).await.expect("list");
        let paths: Vec<&str> = nodes
            .iter()
            .filter_map(|node| node.node_path.as_deref())
            .collect();
        for expected in [
            "decidir",
            "decidir/t1",
            "decidir/e1",
            "lote",
            "lote/b0/p1",
            "lote/b1/p2",
            "arquivos",
        ] {
            assert!(paths.contains(&expected), "missing {expected}: {paths:?}");
        }

        ensure_dag_node(
            &ctx.state.db,
            &run.id,
            "arquivos/0000/analisar",
            Some("arquivos/0000"),
            "analisar",
            "step",
            Some("0000"),
            None,
        )
        .await
        .expect("dynamic node");
        let node = node_by_path(&ctx.state.db, &run.id, "arquivos/0000/analisar")
            .await
            .expect("query")
            .expect("node");
        assert_eq!(node.item_key.as_deref(), Some("0000"));
        assert_eq!(node.parent_path.as_deref(), Some("arquivos/0000"));
    }
}
