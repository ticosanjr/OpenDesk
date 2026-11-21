use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct InboundSource {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub source_type: String,
    pub config_json: String,
    pub target_agent: Option<String>,
    pub target_column: String,
    pub auto_execute: bool,
    pub is_enabled: bool,
    pub last_triggered_at: Option<String>,
    pub last_status: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct OutboundSink {
    pub id: String,
    pub workspace_id: String,
    pub name: String,
    pub sink_type: String,
    pub config_json: String,
    pub is_enabled: bool,
    pub last_run_at: Option<String>,
    pub last_status: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SinkBinding {
    pub id: String,
    pub task_id: Option<String>,
    pub schedule_id: Option<String>,
    pub inbound_source_id: Option<String>,
    pub sink_id: String,
    pub execution_order: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InboundInput {
    pub name: String,
    pub source_type: String,
    #[serde(default)]
    pub config: serde_json::Value,
    pub target_agent: Option<String>,
    pub target_column: Option<String>,
    #[serde(default = "default_true")]
    pub auto_execute: bool,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutboundInput {
    pub name: String,
    pub sink_type: String,
    #[serde(default)]
    pub config: serde_json::Value,
    #[serde(default = "default_true")]
    pub is_enabled: bool,
}

fn default_true() -> bool {
    true
}

const INBOUND_COLUMNS: &str = "id, workspace_id, name, source_type, config_json, target_agent, \
    target_column, auto_execute, is_enabled, last_triggered_at, last_status, last_error, created_at";
const OUTBOUND_COLUMNS: &str = "id, workspace_id, name, sink_type, config_json, is_enabled, \
    last_run_at, last_status, last_error, created_at";
const BINDING_COLUMNS: &str =
    "id, task_id, schedule_id, inbound_source_id, sink_id, execution_order";

fn source_type(value: &str) -> AppResult<String> {
    let value = value.trim().to_uppercase();
    if value == "RSYNC_FOLDER" {
        return Ok("FOLDER_WATCH".into());
    }
    const TYPES: [&str; 9] = [
        "FOLDER_WATCH",
        "OCR_WATCH",
        "AUDIO_STT_WATCH",
        "IMAP_IDLE",
        "WEBHOOK_HTTP",
        "SQL_POLLER",
        "TELEGRAM_BOT",
        "MATRIX_BOT",
        "MATTERMOST_BOT",
    ];
    if TYPES.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(AppError::BadRequest(format!(
            "source_type inválido '{value}'"
        )))
    }
}

fn webhook_secret_ref(config: &serde_json::Value) -> Option<String> {
    config
        .get("secret_ref")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// A webhook without a shared secret would accept any request, including one
/// that starts an agent run through `auto_execute`. Refuse to persist one.
fn ensure_webhook_secret_ref(source_type: &str, config: &serde_json::Value) -> AppResult<()> {
    if source_type == "WEBHOOK_HTTP" && webhook_secret_ref(config).is_none() {
        return Err(AppError::BadRequest(
            "webhook HTTP exige segredo; informe config.secret ou config.secret_ref".into(),
        ));
    }
    Ok(())
}

fn sink_type(value: &str) -> AppResult<String> {
    let value = value.trim().to_uppercase();
    if value == "RSYNC_FOLDER" {
        return Ok("FOLDER_WATCH".into());
    }
    const TYPES: [&str; 9] = [
        "ATOMIC_FS",
        "MINIO_S3",
        "SMTP_EMAIL",
        "TELEGRAM_MSG",
        "MATRIX_MSG",
        "MATTERMOST_MSG",
        "DOC_COMPILER",
        "GIT_COMMIT",
        "SQL_INSERT",
    ];
    if TYPES.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(AppError::BadRequest(format!(
            "sink_type inválido '{value}'"
        )))
    }
}

fn target_column(value: Option<&str>) -> AppResult<String> {
    let value = value.unwrap_or("TODO").trim().to_uppercase();
    let value = if value == "BACKLOG" {
        "TODO".to_string()
    } else {
        value
    };
    const STATUSES: [&str; 6] = [
        "TODO",
        "IN_PROGRESS",
        "IN_REVIEW",
        "DONE",
        "CANCELLED",
        "BLOCKED",
    ];
    if STATUSES.contains(&value.as_str()) {
        Ok(value)
    } else {
        Err(AppError::BadRequest(format!(
            "target_column inválida '{value}'"
        )))
    }
}

pub async fn list_inbound(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<InboundSource>> {
    let query = format!(
        "SELECT {INBOUND_COLUMNS} FROM io_inbound_sources WHERE workspace_id = ? ORDER BY name"
    );
    Ok(sqlx::query_as::<_, InboundSource>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn all_enabled_inbound(pool: &SqlitePool) -> AppResult<Vec<InboundSource>> {
    let query = format!(
        "SELECT {INBOUND_COLUMNS} FROM io_inbound_sources WHERE is_enabled = 1 ORDER BY workspace_id, name"
    );
    Ok(sqlx::query_as::<_, InboundSource>(&query)
        .fetch_all(pool)
        .await?)
}

pub async fn get_inbound(pool: &SqlitePool, id: &str) -> AppResult<InboundSource> {
    let query = format!("SELECT {INBOUND_COLUMNS} FROM io_inbound_sources WHERE id = ?");
    sqlx::query_as::<_, InboundSource>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("inbound {id}")))
}

pub async fn create_inbound(
    pool: &SqlitePool,
    workspace_id: &str,
    input: InboundInput,
) -> AppResult<InboundSource> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("nome do gatilho é obrigatório".into()));
    }
    let id = Uuid::now_v7().to_string();
    let source_type = source_type(&input.source_type)?;
    ensure_webhook_secret_ref(&source_type, &input.config)?;
    let column = target_column(input.target_column.as_deref())?;
    sqlx::query(
        "INSERT INTO io_inbound_sources (
            id, workspace_id, name, source_type, config_json, target_agent,
            target_column, auto_execute, is_enabled
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(name)
    .bind(&source_type)
    .bind(input.config.to_string())
    .bind(input.target_agent.as_deref())
    .bind(&column)
    .bind(input.auto_execute)
    .bind(input.is_enabled)
    .execute(pool)
    .await?;
    get_inbound(pool, &id).await
}

pub async fn update_inbound(
    pool: &SqlitePool,
    id: &str,
    input: InboundInput,
) -> AppResult<InboundSource> {
    let current = get_inbound(pool, id).await?;
    let source_type = source_type(&input.source_type)?;
    ensure_webhook_secret_ref(&source_type, &input.config)?;
    let column = target_column(input.target_column.as_deref())?;
    sqlx::query(
        "UPDATE io_inbound_sources SET name = ?, source_type = ?, config_json = ?,
            target_agent = ?, target_column = ?, auto_execute = ?, is_enabled = ?
         WHERE id = ?",
    )
    .bind(input.name.trim())
    .bind(&source_type)
    .bind(input.config.to_string())
    .bind(input.target_agent.as_deref())
    .bind(&column)
    .bind(input.auto_execute)
    .bind(input.is_enabled)
    .bind(&current.id)
    .execute(pool)
    .await?;
    get_inbound(pool, id).await
}

pub async fn delete_inbound(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM io_inbound_sources WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_inbound(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    error: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE io_inbound_sources SET last_triggered_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
            last_status = ?, last_error = ? WHERE id = ?",
    )
    .bind(status)
    .bind(error)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_outbound(pool: &SqlitePool, workspace_id: &str) -> AppResult<Vec<OutboundSink>> {
    let query = format!(
        "SELECT {OUTBOUND_COLUMNS} FROM io_outbound_sinks WHERE workspace_id = ? ORDER BY name"
    );
    Ok(sqlx::query_as::<_, OutboundSink>(&query)
        .bind(workspace_id)
        .fetch_all(pool)
        .await?)
}

pub async fn get_outbound(pool: &SqlitePool, id: &str) -> AppResult<OutboundSink> {
    let query = format!("SELECT {OUTBOUND_COLUMNS} FROM io_outbound_sinks WHERE id = ?");
    sqlx::query_as::<_, OutboundSink>(&query)
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("sink {id}")))
}

pub async fn create_outbound(
    pool: &SqlitePool,
    workspace_id: &str,
    input: OutboundInput,
) -> AppResult<OutboundSink> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("nome do sink é obrigatório".into()));
    }
    let id = Uuid::now_v7().to_string();
    let sink_type = sink_type(&input.sink_type)?;
    sqlx::query(
        "INSERT INTO io_outbound_sinks (id, workspace_id, name, sink_type, config_json, is_enabled)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(workspace_id)
    .bind(name)
    .bind(&sink_type)
    .bind(input.config.to_string())
    .bind(input.is_enabled)
    .execute(pool)
    .await?;
    get_outbound(pool, &id).await
}

pub async fn update_outbound(
    pool: &SqlitePool,
    id: &str,
    input: OutboundInput,
) -> AppResult<OutboundSink> {
    let current = get_outbound(pool, id).await?;
    let sink_type = sink_type(&input.sink_type)?;
    sqlx::query(
        "UPDATE io_outbound_sinks SET name = ?, sink_type = ?, config_json = ?, is_enabled = ?
         WHERE id = ?",
    )
    .bind(input.name.trim())
    .bind(&sink_type)
    .bind(input.config.to_string())
    .bind(input.is_enabled)
    .bind(&current.id)
    .execute(pool)
    .await?;
    get_outbound(pool, id).await
}

pub async fn delete_outbound(pool: &SqlitePool, id: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM io_outbound_sinks WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_outbound(
    pool: &SqlitePool,
    id: &str,
    status: &str,
    error: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE io_outbound_sinks SET last_run_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
            last_status = ?, last_error = ? WHERE id = ?",
    )
    .bind(status)
    .bind(error)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn bindings_for_task(pool: &SqlitePool, task_id: &str) -> AppResult<Vec<SinkBinding>> {
    let query = format!(
        "SELECT {BINDING_COLUMNS} FROM task_sink_bindings WHERE task_id = ? ORDER BY execution_order"
    );
    Ok(sqlx::query_as::<_, SinkBinding>(&query)
        .bind(task_id)
        .fetch_all(pool)
        .await?)
}

pub async fn bindings_for_schedule(
    pool: &SqlitePool,
    schedule_id: &str,
) -> AppResult<Vec<SinkBinding>> {
    let query = format!(
        "SELECT {BINDING_COLUMNS} FROM task_sink_bindings WHERE schedule_id = ? ORDER BY execution_order"
    );
    Ok(sqlx::query_as::<_, SinkBinding>(&query)
        .bind(schedule_id)
        .fetch_all(pool)
        .await?)
}

pub async fn set_task_bindings(
    pool: &SqlitePool,
    task_id: &str,
    sink_ids: &[String],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM task_sink_bindings WHERE task_id = ?")
        .bind(task_id)
        .execute(&mut *tx)
        .await?;
    for (index, sink_id) in sink_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO task_sink_bindings (id, task_id, sink_id, execution_order)
             VALUES (?, ?, ?, ?)",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(task_id)
        .bind(sink_id)
        .bind(index as i64 + 1)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn set_schedule_bindings(
    pool: &SqlitePool,
    schedule_id: &str,
    sink_ids: &[String],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM task_sink_bindings WHERE schedule_id = ?")
        .bind(schedule_id)
        .execute(&mut *tx)
        .await?;
    for (index, sink_id) in sink_ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO task_sink_bindings (id, schedule_id, sink_id, execution_order)
             VALUES (?, ?, ?, ?)",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(schedule_id)
        .bind(sink_id)
        .bind(index as i64 + 1)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Sinks that must run for a run. Task bindings win; schedule bindings are used
/// when the run belongs to a routine.
pub async fn sinks_for_run(
    pool: &SqlitePool,
    task_id: Option<&str>,
    schedule_id: Option<&str>,
) -> AppResult<Vec<OutboundSink>> {
    let ids: Vec<String> = if let Some(task_id) = task_id {
        sqlx::query_scalar::<_, String>(
            "SELECT sink_id FROM task_sink_bindings WHERE task_id = ? ORDER BY execution_order",
        )
        .bind(task_id)
        .fetch_all(pool)
        .await?
    } else if let Some(schedule_id) = schedule_id {
        sqlx::query_scalar::<_, String>(
            "SELECT sink_id FROM task_sink_bindings WHERE schedule_id = ? ORDER BY execution_order",
        )
        .bind(schedule_id)
        .fetch_all(pool)
        .await?
    } else {
        Vec::new()
    };
    let mut sinks = Vec::new();
    for id in ids {
        if let Ok(sink) = get_outbound(pool, &id).await
            && sink.is_enabled
        {
            sinks.push(sink);
        }
    }
    Ok(sinks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let temp = TempDir::new().expect("temp");
        let pool = crate::db::init(&temp.path().join("io.db"))
            .await
            .expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        (temp, pool)
    }

    #[tokio::test]
    async fn inbound_and_outbound_crud_and_bindings() {
        let (_temp, pool) = setup().await;
        let inbound = create_inbound(
            &pool,
            "ws-1",
            InboundInput {
                name: "Dropzone".into(),
                source_type: "FOLDER_WATCH".into(),
                config: serde_json::json!({"path":"input/dropzone"}),
                target_agent: Some("build".into()),
                target_column: Some("TODO".into()),
                auto_execute: true,
                is_enabled: true,
            },
        )
        .await
        .expect("inbound");
        let sink = create_outbound(
            &pool,
            "ws-1",
            OutboundInput {
                name: "PDF".into(),
                sink_type: "DOC_COMPILER".into(),
                config: serde_json::json!({"format":"pdf"}),
                is_enabled: true,
            },
        )
        .await
        .expect("sink");
        let task = crate::db::tasks::create(
            &pool,
            "ws-1",
            crate::db::tasks::CreateTaskInput {
                title: "Tarefa IO".into(),
                description: "x".into(),
                execution_mode: None,
                priority: None,
                assigned_agent: None,
                preferred_model: None,
                preferred_harness: None,
                effort_level: None,
                care_level: Some("standard".into()),
                notify_email: None,
                export_to_gdocs: false,
                tags: vec![],
                origin: None,
                parent_task_id: None,
                dedupe: None,
            },
        )
        .await
        .expect("task");
        set_task_bindings(&pool, &task.id, std::slice::from_ref(&sink.id))
            .await
            .expect("bind");
        assert_eq!(
            sinks_for_run(&pool, Some(&task.id), None)
                .await
                .expect("sinks")
                .len(),
            1
        );
        assert_eq!(list_inbound(&pool, "ws-1").await.expect("list").len(), 1);
        assert_eq!(inbound.source_type, "FOLDER_WATCH");
    }
}
