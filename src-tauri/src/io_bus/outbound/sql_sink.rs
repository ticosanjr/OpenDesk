use crate::{
    db::io_bus::OutboundSink,
    error::{AppError, AppResult},
    io_bus::{IoContext, RunOutput, config_value, outbound::output_root},
};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default)]
    database_url: Option<String>,
    #[serde(default)]
    database_url_ref: Option<String>,
    query: String,
    #[serde(default)]
    artifact: Option<String>,
    #[serde(default)]
    columns: Vec<String>,
}

pub async fn run(
    ctx: &IoContext,
    sink: &OutboundSink,
    output: &RunOutput,
) -> AppResult<Vec<String>> {
    let config: Config = config_value(sink)?;
    let database_url = match config.database_url {
        Some(url) if !url.trim().is_empty() => url,
        _ => crate::io_bus::secret(&ctx.paths, config.database_url_ref.as_deref(), None)?,
    };
    let source = config
        .artifact
        .as_deref()
        .map(|path| {
            let path = Path::new(path);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                output_root(output).join(path)
            }
        })
        .or_else(|| {
            output
                .artifacts
                .iter()
                .map(|artifact| Path::new(&artifact.absolute_path).to_path_buf())
                .find(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        })
        .ok_or_else(|| AppError::BadRequest("SQL_INSERT exige um artefato JSON".into()))?;
    let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&source)?)
        .map_err(|error| AppError::BadRequest(format!("JSON inválido: {error}")))?;
    let rows = value
        .as_array()
        .cloned()
        .unwrap_or_else(|| vec![value.clone()]);

    sqlx::any::install_default_drivers();
    let pool = sqlx::any::AnyPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .map_err(|error| AppError::Daemon(format!("conexão SQL: {error}")))?;
    let mut affected = 0u64;
    for row in rows {
        let mut query = sqlx::query(&config.query);
        for column in &config.columns {
            let value = row.get(column).cloned().unwrap_or(serde_json::Value::Null);
            query = match value {
                serde_json::Value::Null => query.bind(Option::<String>::None),
                serde_json::Value::Bool(value) => query.bind(value),
                serde_json::Value::Number(value) => {
                    if let Some(integer) = value.as_i64() {
                        query.bind(integer)
                    } else {
                        query.bind(value.as_f64().unwrap_or_default())
                    }
                }
                serde_json::Value::String(value) => query.bind(value),
                other => query.bind(other.to_string()),
            };
        }
        let result = query
            .execute(&pool)
            .await
            .map_err(|error| AppError::Daemon(format!("SQL insert: {error}")))?;
        affected += result.rows_affected();
    }
    pool.close().await;
    Ok(vec![format!("{affected} linha(s) afetada(s)")])
}
