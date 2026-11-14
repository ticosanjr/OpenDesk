use crate::error::AppResult;
use sqlx::{FromRow, SqlitePool};
use std::collections::HashMap;

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow)]
pub struct RecipeFileState {
    pub recipe_id: String,
    pub trigger_path: String,
    pub file_path: String,
    pub fingerprint: String,
    pub status: String,
    pub run_id: Option<String>,
    pub processed_at: String,
}

const COLUMNS: &str =
    "recipe_id, trigger_path, file_path, fingerprint, status, run_id, processed_at";

/// Loads all known files for one recipe trigger in a single query. Folder
/// scans call this once instead of querying SQLite per file.
pub async fn list_for_trigger(
    pool: &SqlitePool,
    recipe_id: &str,
    trigger_path: &str,
) -> AppResult<HashMap<String, RecipeFileState>> {
    let query = format!(
        "SELECT {COLUMNS} FROM recipe_file_state
         WHERE recipe_id = ? AND trigger_path = ?"
    );
    let rows = sqlx::query_as::<_, RecipeFileState>(&query)
        .bind(recipe_id)
        .bind(trigger_path)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.file_path.clone(), row))
        .collect())
}

pub async fn upsert(
    pool: &SqlitePool,
    recipe_id: &str,
    trigger_path: &str,
    file_path: &str,
    fingerprint: &str,
    status: &str,
    run_id: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO recipe_file_state
            (recipe_id, trigger_path, file_path, fingerprint, status, run_id, processed_at)
         VALUES (?, ?, ?, ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ','now'))
         ON CONFLICT(recipe_id, trigger_path, file_path) DO UPDATE SET
            fingerprint = excluded.fingerprint,
            status = excluded.status,
            run_id = excluded.run_id,
            processed_at = excluded.processed_at",
    )
    .bind(recipe_id)
    .bind(trigger_path)
    .bind(file_path)
    .bind(fingerprint)
    .bind(status)
    .bind(run_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Backfill helper for the first scan after an upgrade: maps trigger files
/// already present in recipe_runs to the latest run start time.
pub async fn latest_run_files(
    pool: &SqlitePool,
    recipe_id: &str,
) -> AppResult<HashMap<String, String>> {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT trigger_context_json, started_at FROM recipe_runs
         WHERE recipe_id = ? ORDER BY started_at DESC",
    )
    .bind(recipe_id)
    .fetch_all(pool)
    .await?;
    let mut files = HashMap::new();
    for (context, started_at) in rows {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&context) else {
            continue;
        };
        let Some(path) = value.get("file_path").and_then(serde_json::Value::as_str) else {
            continue;
        };
        files.entry(path.to_string()).or_insert(started_at);
    }
    Ok(files)
}

pub async fn update_status_by_run(pool: &SqlitePool, run_id: &str, status: &str) -> AppResult<()> {
    sqlx::query("UPDATE recipe_file_state SET status = ? WHERE run_id = ?")
        .bind(status)
        .bind(run_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[allow(dead_code)]
pub async fn delete_for_recipe(pool: &SqlitePool, recipe_id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM recipe_file_state WHERE recipe_id = ?")
        .bind(recipe_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn stores_and_updates_file_fingerprints() {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("files.db"))
            .await
            .expect("db");
        upsert(
            &pool,
            "recipe",
            "input",
            "/tmp/input/a.pdf",
            "100:10",
            "RUNNING",
            Some("run-1"),
        )
        .await
        .expect("upsert");
        let states = list_for_trigger(&pool, "recipe", "input")
            .await
            .expect("list");
        let state = states.get("/tmp/input/a.pdf").expect("state");
        assert_eq!(state.fingerprint, "100:10");
        assert_eq!(state.status, "RUNNING");

        update_status_by_run(&pool, "run-1", "COMPLETED")
            .await
            .expect("status");
        let states = list_for_trigger(&pool, "recipe", "input")
            .await
            .expect("list");
        assert_eq!(
            states.get("/tmp/input/a.pdf").expect("state").status,
            "COMPLETED"
        );

        delete_for_recipe(&pool, "recipe").await.expect("delete");
        assert!(
            list_for_trigger(&pool, "recipe", "input")
                .await
                .expect("list")
                .is_empty()
        );

        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws', 'WS', '/tmp/ws')")
            .execute(&pool)
            .await
            .expect("workspace");
        sqlx::query(
            "INSERT INTO recipes_index (id, workspace_id, name, file_path, trigger_type)
             VALUES ('recipe', 'ws', 'Recipe', '/tmp/recipe.yaml', 'FOLDER_WATCH')",
        )
        .execute(&pool)
        .await
        .expect("recipe");
        for (id, started, path) in [
            ("run-old", "2026-01-01T10:00:00Z", "/tmp/input/a.pdf"),
            ("run-new", "2026-01-02T10:00:00Z", "/tmp/input/a.pdf"),
            ("run-b", "2026-01-01T11:00:00Z", "/tmp/input/b.pdf"),
        ] {
            sqlx::query(
                "INSERT INTO recipe_runs
                    (id, recipe_id, workspace_id, status, trigger_context_json, output_dir_path, started_at)
                 VALUES (?, 'recipe', 'ws', 'COMPLETED', ?, '/tmp/out', ?)",
            )
            .bind(id)
            .bind(serde_json::json!({ "file_path": path }).to_string())
            .bind(started)
            .execute(&pool)
            .await
            .expect("run");
        }
        let previous = latest_run_files(&pool, "recipe").await.expect("latest");
        assert_eq!(
            previous.get("/tmp/input/a.pdf").map(String::as_str),
            Some("2026-01-02T10:00:00Z")
        );
        assert_eq!(
            previous.get("/tmp/input/b.pdf").map(String::as_str),
            Some("2026-01-01T11:00:00Z")
        );
    }
}
