use crate::{
    app_state::AppState,
    db::io_bus::{self, InboundSource},
    error::{AppError, AppResult},
    io_bus::{NormalizedTaskEvent, source_config},
};
use serde::Deserialize;
use sqlx::{Column, Row};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Deserialize)]
pub struct PollerConfig {
    #[serde(default)]
    pub database_url: Option<String>,
    #[serde(default)]
    pub database_url_ref: Option<String>,
    pub query: String,
    #[serde(default)]
    pub confirm_query: Option<String>,
    #[serde(default)]
    pub id_column: Option<String>,
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
}
fn default_interval() -> u64 {
    60
}

pub async fn run(
    state: Arc<AppState>,
    source: InboundSource,
    cancel: CancellationToken,
) -> AppResult<()> {
    let config: PollerConfig = source_config(&source)?;
    loop {
        if cancel.is_cancelled() {
            break;
        }
        if let Err(error) = poll_once(&state, &source, &config).await {
            tracing::warn!(source = %source.id, error = %error, "SQL poller failed");
            let _ =
                io_bus::mark_inbound(&state.db, &source.id, "FAILED", Some(&format!("{error:#}")))
                    .await;
        }
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = tokio::time::sleep(Duration::from_secs(config.interval_secs.clamp(5, 3600))) => {}
        }
    }
    Ok(())
}

async fn poll_once(
    state: &Arc<AppState>,
    source: &InboundSource,
    config: &PollerConfig,
) -> AppResult<()> {
    let database_url = database_url(state, config)?;
    sqlx::any::install_default_drivers();
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .map_err(|error| AppError::Daemon(format!("SQL poller: {error}")))?;
    let rows = sqlx::query(&config.query)
        .fetch_all(&pool)
        .await
        .map_err(|error| AppError::Daemon(format!("SQL query: {error}")))?;
    for row in rows {
        let json = row_json(&row);
        let id = config
            .id_column
            .as_deref()
            .and_then(|column| json.get(column).cloned())
            .map(|value| match value {
                serde_json::Value::String(value) => value,
                other => other.to_string(),
            })
            .unwrap_or_else(|| json.to_string());
        let title = ["title", "subject", "name"]
            .iter()
            .find_map(|key| json.get(*key).and_then(|value| value.as_str()))
            .unwrap_or("Registro SQL")
            .to_string();
        let event = NormalizedTaskEvent {
            source_id: source.id.clone(),
            external_id: id.clone(),
            title,
            body_markdown: format!("# Registro SQL\n\n```json\n{}\n```", json),
            priority: json.get("priority").and_then(|value| value.as_i64()),
            tags: vec!["sql-poller".into()],
            attachments: vec![],
            metadata: json,
        };
        crate::io_bus::ingest(state, source, event).await?;
        if let Some(confirm) = config.confirm_query.as_deref() {
            sqlx::query(confirm)
                .bind(id)
                .execute(&pool)
                .await
                .map_err(|error| AppError::Daemon(format!("SQL confirm: {error}")))?;
        }
    }
    let _ = io_bus::mark_inbound(&state.db, &source.id, "TRIGGERED", None).await;
    pool.close().await;
    Ok(())
}

fn database_url(state: &Arc<AppState>, config: &PollerConfig) -> AppResult<String> {
    if let Some(url) = config
        .database_url
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(url.to_string());
    }
    crate::io_bus::secret(&state.paths, config.database_url_ref.as_deref(), None)
}

fn row_json(row: &sqlx::any::AnyRow) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    for column in row.columns() {
        let name = column.name();
        let value = row
            .try_get::<Option<String>, _>(name)
            .ok()
            .flatten()
            .map(serde_json::Value::String)
            .or_else(|| {
                row.try_get::<Option<i64>, _>(name)
                    .ok()
                    .flatten()
                    .map(|value| serde_json::json!(value))
            })
            .or_else(|| {
                row.try_get::<Option<f64>, _>(name)
                    .ok()
                    .flatten()
                    .map(|value| serde_json::json!(value))
            })
            .or_else(|| {
                row.try_get::<Option<bool>, _>(name)
                    .ok()
                    .flatten()
                    .map(|value| serde_json::json!(value))
            })
            .unwrap_or(serde_json::Value::Null);
        object.insert(name.to_string(), value);
    }
    serde_json::Value::Object(object)
}
