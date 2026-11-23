use crate::error::AppResult;

use sqlx::SqlitePool;

use super::RecipeRunNode;

const NODE_COLUMNS: &str = "id, run_id, node_index, node_path, parent_path, node_id, kind, \
    item_key, join_policy, status, attempts, session_id, output_json, error_message, \
    started_at, finished_at";

pub async fn sync_run_nodes(
    pool: &SqlitePool,
    run_id: &str,
    flow: &[crate::parser::FlowItem],
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
    flow: &[crate::parser::FlowItem],
    parent: Option<&str>,
    seeds: &mut Vec<NodeSeed>,
) {
    use crate::parser::FlowItem;
    for (index, item) in flow.iter().enumerate() {
        let (node_id, kind) = crate::parser::node_descriptor(item, index);
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
            FlowItem::Map(_) | FlowItem::Recipe(_) | FlowItem::Loop(_) | FlowItem::Switch(_) => {
                seeds.push(NodeSeed {
                    path,
                    parent: parent.map(str::to_string),
                    node_id,
                    kind,
                    item_key: None,
                    join_policy: None,
                });
                // Map items and sub-recipe children are materialized at runtime.
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

pub async fn reset_node_subtree(pool: &SqlitePool, run_id: &str, path: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE recipe_run_nodes
         SET status = 'PENDING', error_message = NULL, finished_at = NULL
         WHERE run_id = ? AND (node_path = ? OR node_path LIKE ?)",
    )
    .bind(run_id)
    .bind(path)
    .bind(format!("{path}/%"))
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
