//! Portable SQLite snapshot for a single workspace.
//!
//! Tables are discovered at runtime so new workspace-scoped tables are backed
//! up without editing a hardcoded list. Each row is stored as JSON in column
//! order; BLOBs use a `{"$blob":"<base64>"}` wrapper.

use crate::{
    error::{AppError, AppResult},
    paths::AppPaths,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sqlx::{Row, Sqlite, SqliteConnection, SqlitePool, TypeInfo, ValueRef, sqlite::SqliteRow};
use std::collections::{BTreeMap, HashMap};

pub(super) const STATE_ENTRY: &str = "state/workspace-state.json";
pub(super) const LEGACY_STATE_ENTRY: &str = "db/workspace_state.json";
pub(super) const RUN_LOGS_PREFIX: &str = "db/run_logs/";
pub(super) const SECRETS_ENTRY: &str = "db/secrets.enc";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableState {
    pub name: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceState {
    pub tables: Vec<TableState>,
}

impl WorkspaceState {
    pub fn counts(&self) -> BTreeMap<String, u64> {
        self.tables
            .iter()
            .map(|table| (table.name.clone(), table.rows.len() as u64))
            .collect()
    }
}

fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn value_ref(row: &SqliteRow, index: usize) -> AppResult<Value> {
    let raw = row
        .try_get_raw(index)
        .map_err(|error| AppError::Internal(format!("snapshot column {index}: {error}")))?;
    if raw.is_null() {
        return Ok(Value::Null);
    }
    match raw.type_info().name() {
        "INTEGER" | "BOOLEAN" => row
            .try_get::<i64, _>(index)
            .map(Value::from)
            .map_err(|error| AppError::Internal(error.to_string())),
        "REAL" => row
            .try_get::<f64, _>(index)
            .map(Value::from)
            .map_err(|error| AppError::Internal(error.to_string())),
        "BLOB" => row
            .try_get::<Vec<u8>, _>(index)
            .map(|bytes| {
                let mut object = Map::new();
                object.insert("$blob".into(), Value::String(STANDARD.encode(bytes)));
                Value::Object(object)
            })
            .map_err(|error| AppError::Internal(error.to_string())),
        _ => row
            .try_get::<String, _>(index)
            .map(Value::String)
            .map_err(|error| AppError::Internal(error.to_string())),
    }
}

async fn columns(
    connection: &mut SqliteConnection,
    table: &str,
) -> AppResult<Vec<(String, String, i64)>> {
    let pragma = format!("PRAGMA table_info({})", quote(table));
    let rows = sqlx::query(&pragma)
        .fetch_all(&mut *connection)
        .await
        .map_err(|error| AppError::Internal(format!("table_info {table}: {error}")))?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.try_get::<String, _>("name").unwrap_or_default(),
                row.try_get::<String, _>("type").unwrap_or_default(),
                row.try_get::<i64, _>("pk").unwrap_or(0),
            )
        })
        .collect())
}

/// Columns that can carry plaintext credentials. Encrypted Vault columns and
/// `*_secret_ref` references stay in the snapshot: they are not plaintext.
fn redacted_column(name: &str) -> bool {
    let name = name.to_lowercase();
    if name.ends_with("_ref") || name.ends_with("_ciphertext") || name.ends_with("_nonce") {
        return false;
    }
    name == "sync_token"
        || name == "token"
        || name == "token_hash"
        || name == "password"
        || name == "api_key"
        || name == "access_token"
        || name == "refresh_token"
        || name.ends_with("_password")
        || name.ends_with("_api_key")
        || name.ends_with("_access_token")
        || name.ends_with("_refresh_token")
}

fn is_uuid_like(value: &str) -> bool {
    uuid::Uuid::parse_str(value).is_ok()
}

/// Discovers every table that carries a `workspace_id` column.
pub(super) async fn workspace_tables(pool: &SqlitePool) -> AppResult<Vec<String>> {
    let names = sqlx::query_scalar::<_, String>(
        "SELECT name FROM sqlite_master
         WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> '_sqlx_migrations'
         ORDER BY name",
    )
    .fetch_all(pool)
    .await?;
    let mut tables = Vec::new();
    for name in names {
        let mut connection = pool.acquire().await?;
        let info = columns(&mut connection, &name).await?;
        drop(connection);
        if info.iter().any(|(column, _, _)| column == "workspace_id") {
            tables.push(name);
        }
    }
    Ok(tables)
}

pub(super) async fn export(pool: &SqlitePool, workspace_id: &str) -> AppResult<WorkspaceState> {
    let tables = workspace_tables(pool).await?;
    let mut connection = pool.acquire().await?;
    let mut output = Vec::new();
    for table in tables {
        if table == "workspaces" {
            continue;
        }
        let info = columns(&mut connection, &table).await?;
        let column_names = info
            .iter()
            .map(|(name, _, _)| name.clone())
            .collect::<Vec<_>>();
        let query = format!("SELECT * FROM {} WHERE workspace_id = ?", quote(&table));
        let rows = sqlx::query(&query)
            .bind(workspace_id)
            .fetch_all(&mut *connection)
            .await
            .map_err(|error| AppError::Internal(format!("snapshot {table}: {error}")))?;
        let redacted = column_names
            .iter()
            .map(|column| redacted_column(column))
            .collect::<Vec<_>>();
        let mut data = Vec::with_capacity(rows.len());
        for row in rows {
            let mut values = Vec::with_capacity(column_names.len());
            for (index, is_redacted) in redacted.iter().enumerate() {
                if *is_redacted {
                    values.push(Value::Null);
                } else {
                    values.push(value_ref(&row, index)?);
                }
            }
            data.push(values);
        }
        output.push(TableState {
            name: table,
            columns: column_names,
            rows: data,
        });
    }
    Ok(WorkspaceState { tables: output })
}

fn bind<'a>(
    query: sqlx::query::Query<'a, Sqlite, sqlx::sqlite::SqliteArguments<'a>>,
    value: &Value,
) -> sqlx::query::Query<'a, Sqlite, sqlx::sqlite::SqliteArguments<'a>> {
    match value {
        Value::Null => query.bind(None::<String>),
        Value::Bool(value) => query.bind(if *value { 1i64 } else { 0i64 }),
        Value::Number(number) => {
            if let Some(value) = number.as_i64() {
                query.bind(value)
            } else {
                query.bind(number.as_f64().unwrap_or_default())
            }
        }
        Value::String(value) => query.bind(value.clone()),
        Value::Object(object) => match object.get("$blob").and_then(Value::as_str) {
            Some(encoded) => match STANDARD.decode(encoded) {
                Ok(bytes) => query.bind(bytes),
                Err(_) => query.bind(Value::Null.to_string()),
            },
            None => query.bind(value.to_string()),
        },
        Value::Array(_) => query.bind(value.to_string()),
    }
}

/// Remaps every string primary key plus `*_id` references so a restored
/// workspace never collides with the original one on the same host.
pub(super) fn remap_ids(state: &mut WorkspaceState, workspace_id: &str) {
    let mut mapping: HashMap<String, String> = HashMap::new();
    for table in &state.tables {
        let Some(index) = table.columns.iter().position(|column| column == "id") else {
            continue;
        };
        for row in &table.rows {
            if let Some(Value::String(old)) = row.get(index)
                && is_uuid_like(old)
                && !mapping.contains_key(old)
            {
                mapping.insert(old.clone(), uuid::Uuid::now_v7().to_string());
            }
        }
    }
    for table in &mut state.tables {
        for (column_index, column) in table.columns.iter().enumerate() {
            let should_remap = column == "id" || column.ends_with("_id");
            for row in &mut table.rows {
                if let Some(Value::String(value)) = row.get_mut(column_index) {
                    if column == "workspace_id" {
                        *value = workspace_id.to_string();
                    } else if should_remap && let Some(replacement) = mapping.get(value.as_str()) {
                        *value = replacement.clone();
                    }
                }
            }
        }
    }
}

/// Rewrites absolute paths recorded in rows (output dirs, artifacts, logs) when
/// a workspace is restored under a new root.
pub(super) fn remap_paths(state: &mut WorkspaceState, old_root: &str, new_root: &str) {
    let old_root = old_root.trim_end_matches('/');
    if old_root.is_empty() || old_root == new_root.trim_end_matches('/') {
        return;
    }
    for table in &mut state.tables {
        for row in &mut table.rows {
            for value in row {
                let Value::String(text) = value else {
                    continue;
                };
                if text == old_root {
                    *text = new_root.to_string();
                } else if let Some(rest) = text.strip_prefix(old_root)
                    && rest.starts_with('/')
                {
                    *text = format!("{}{rest}", new_root.trim_end_matches('/'));
                }
            }
        }
    }
}

pub(super) async fn restore(
    connection: &mut SqliteConnection,
    state: &WorkspaceState,
    workspace_id: &str,
) -> AppResult<()> {
    for table in &state.tables {
        if table.name == "workspaces" || table.columns.is_empty() {
            continue;
        }
        let placeholders = vec!["?"; table.columns.len()].join(", ");
        let columns = table
            .columns
            .iter()
            .map(|column| quote(column))
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "INSERT OR REPLACE INTO {} ({columns}) VALUES ({placeholders})",
            quote(&table.name)
        );
        for row in &table.rows {
            if row.len() != table.columns.len() {
                return Err(AppError::BadRequest(format!(
                    "snapshot inválido para {}",
                    table.name
                )));
            }
            let mut query = sqlx::query(&sql);
            for (index, value) in row.iter().enumerate() {
                let value = if table.columns[index] == "workspace_id" {
                    &Value::String(workspace_id.to_string())
                } else {
                    value
                };
                query = bind(query, value);
            }
            query
                .execute(&mut *connection)
                .await
                .map_err(|error| AppError::Internal(format!("restore {}: {error}", table.name)))?;
        }
    }
    Ok(())
}

pub(super) fn sha256_bytes(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Reads `<run_id>.jsonl` files referenced by the workspace's runs.
pub(super) fn run_log_files(paths: &AppPaths, run_ids: &[String]) -> Vec<(String, Vec<u8>)> {
    let mut output = Vec::new();
    for run_id in run_ids {
        if run_id.contains('/') || run_id.contains('\\') || run_id.contains("..") {
            continue;
        }
        let name = format!("{run_id}.jsonl");
        let path = paths.run_logs_dir.join(&name);
        if let Ok(bytes) = std::fs::read(&path) {
            output.push((format!("{RUN_LOGS_PREFIX}{run_id}.jsonl"), bytes));
        }
    }
    output
}
