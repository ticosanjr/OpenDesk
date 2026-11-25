//! CRUD for workspace/global script tools exposed through the local MCP.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ScriptTool {
    pub id: String,
    pub workspace_id: Option<String>,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub agent_prompt: String,
    pub runtime: String,
    pub custom_command: Option<String>,
    pub script_body: String,
    pub input_schema_json: String,
    pub env_secret_refs_json: String,
    pub timeout_secs: i64,
    pub require_approval: bool,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateScriptToolInput {
    #[serde(default)]
    pub workspace_id: Option<String>,
    pub slug: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub agent_prompt: String,
    pub runtime: String,
    #[serde(default)]
    pub custom_command: Option<String>,
    pub script_body: String,
    #[serde(default = "default_schema")]
    pub input_schema_json: String,
    #[serde(default = "default_env")]
    pub env_secret_refs_json: String,
    #[serde(default = "default_timeout")]
    pub timeout_secs: i64,
    #[serde(default)]
    pub require_approval: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UpdateScriptToolInput {
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub agent_prompt: Option<String>,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub custom_command: Option<Option<String>>,
    #[serde(default)]
    pub script_body: Option<String>,
    #[serde(default)]
    pub input_schema_json: Option<String>,
    #[serde(default)]
    pub env_secret_refs_json: Option<String>,
    #[serde(default)]
    pub timeout_secs: Option<i64>,
    #[serde(default)]
    pub require_approval: Option<bool>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

fn default_schema() -> String {
    r#"{"type":"object","properties":{}}"#.into()
}
fn default_env() -> String {
    "{}".into()
}
fn default_timeout() -> i64 {
    30
}
fn default_true() -> bool {
    true
}

const COLUMNS: &str = "id, workspace_id, slug, name, description, agent_prompt, runtime, \
    custom_command, script_body, input_schema_json, env_secret_refs_json, timeout_secs, \
    require_approval, enabled, created_at, updated_at";

pub async fn list(pool: &SqlitePool, workspace_id: Option<&str>) -> AppResult<Vec<ScriptTool>> {
    let query = format!(
        "SELECT {COLUMNS} FROM script_tools
         WHERE workspace_id IS NULL OR workspace_id = ?
         ORDER BY workspace_id IS NULL DESC, name COLLATE NOCASE"
    );
    Ok(sqlx::query_as::<_, ScriptTool>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn list_enabled(
    pool: &SqlitePool,
    workspace_id: Option<&str>,
) -> AppResult<Vec<ScriptTool>> {
    Ok(list(pool, workspace_id)
        .await?
        .into_iter()
        .filter(|tool| tool.enabled)
        .collect())
}

pub async fn get(pool: &SqlitePool, id: &str) -> AppResult<ScriptTool> {
    let query = format!("SELECT {COLUMNS} FROM script_tools WHERE id = ?");
    sqlx::query_as::<_, ScriptTool>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("script tool {id}")))
}

pub async fn find_by_slug(
    pool: &SqlitePool,
    workspace_id: Option<&str>,
    slug: &str,
) -> AppResult<Option<ScriptTool>> {
    let query = format!(
        "SELECT {COLUMNS} FROM script_tools
         WHERE slug = ? AND (workspace_id IS NULL OR workspace_id = ?)
         ORDER BY workspace_id IS NULL ASC LIMIT 1"
    );
    Ok(sqlx::query_as::<_, ScriptTool>(&query)
        .bind(slug)
        .bind(workspace_id)
        .fetch_optional(pool)
        .await?)
}

pub async fn create(pool: &SqlitePool, input: CreateScriptToolInput) -> AppResult<ScriptTool> {
    let id = Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO script_tools
            (id, workspace_id, slug, name, description, agent_prompt, runtime,
             custom_command, script_body, input_schema_json, env_secret_refs_json,
             timeout_secs, require_approval, enabled)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&input.workspace_id)
    .bind(&input.slug)
    .bind(&input.name)
    .bind(&input.description)
    .bind(&input.agent_prompt)
    .bind(&input.runtime)
    .bind(&input.custom_command)
    .bind(&input.script_body)
    .bind(&input.input_schema_json)
    .bind(&input.env_secret_refs_json)
    .bind(input.timeout_secs)
    .bind(input.require_approval)
    .bind(input.enabled)
    .execute(pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(db) if db.is_unique_violation() => AppError::Conflict(format!(
            "já existe uma ferramenta com o slug '{}'",
            input.slug
        )),
        other => AppError::Db(other),
    })?;
    get(pool, &id).await
}

pub async fn update(
    pool: &SqlitePool,
    id: &str,
    input: UpdateScriptToolInput,
) -> AppResult<ScriptTool> {
    let current = get(pool, id).await?;
    let slug = input.slug.unwrap_or(current.slug);
    let name = input.name.unwrap_or(current.name);
    let description = input.description.unwrap_or(current.description);
    let agent_prompt = input.agent_prompt.unwrap_or(current.agent_prompt);
    let runtime = input.runtime.unwrap_or(current.runtime);
    let custom_command = input.custom_command.unwrap_or(current.custom_command);
    let script_body = input.script_body.unwrap_or(current.script_body);
    let input_schema_json = input.input_schema_json.unwrap_or(current.input_schema_json);
    let env_secret_refs_json = input
        .env_secret_refs_json
        .unwrap_or(current.env_secret_refs_json);
    let timeout_secs = input.timeout_secs.unwrap_or(current.timeout_secs);
    let require_approval = input.require_approval.unwrap_or(current.require_approval);
    let enabled = input.enabled.unwrap_or(current.enabled);
    sqlx::query(
        "UPDATE script_tools SET
            slug = ?, name = ?, description = ?, agent_prompt = ?, runtime = ?,
            custom_command = ?, script_body = ?, input_schema_json = ?,
            env_secret_refs_json = ?, timeout_secs = ?, require_approval = ?,
            enabled = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(&slug)
    .bind(&name)
    .bind(&description)
    .bind(&agent_prompt)
    .bind(&runtime)
    .bind(&custom_command)
    .bind(&script_body)
    .bind(&input_schema_json)
    .bind(&env_secret_refs_json)
    .bind(timeout_secs)
    .bind(require_approval)
    .bind(enabled)
    .bind(id)
    .execute(pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            AppError::Conflict(format!("já existe uma ferramenta com o slug '{slug}'"))
        }
        other => AppError::Db(other),
    })?;
    get(pool, id).await
}

pub async fn delete(pool: &SqlitePool, id: &str) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM script_tools WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("script tool {id}")));
    }
    Ok(())
}

pub async fn set_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> AppResult<ScriptTool> {
    sqlx::query(
        "UPDATE script_tools SET enabled = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(enabled)
    .bind(id)
    .execute(pool)
    .await?;
    get(pool, id).await
}
