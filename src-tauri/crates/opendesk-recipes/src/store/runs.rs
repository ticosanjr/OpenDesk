use crate::error::{AppError, AppResult};
use crate::parser::Recipe;

use sqlx::SqlitePool;
use uuid::Uuid;

use super::RecipeRun;

const RUN_COLUMNS: &str = "id, recipe_id, workspace_id, task_id, status, current_step_id, \
    trigger_context_json, step_outputs_json, output_dir_path, dry_run, cancel_requested, \
    error_message, started_at, completed_at";

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
