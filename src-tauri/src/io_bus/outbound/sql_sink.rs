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
    let database_url = crate::io_bus::secret(&ctx.paths, config.database_url_ref.as_deref())?;
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

    let pool = crate::io_bus::sql::connect(&database_url).await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::io_bus::OutboundSink;
    use crate::io_bus::{IoContext, RunArtifact, RunOutput};
    use serde_json::json;

    fn sink(config: serde_json::Value) -> OutboundSink {
        OutboundSink {
            id: "sink-1".into(),
            workspace_id: "ws".into(),
            name: "SQL".into(),
            sink_type: "SQL_INSERT".into(),
            config_json: config.to_string(),
            is_enabled: true,
            last_run_at: None,
            last_status: None,
            last_error: None,
            created_at: "2026-11-13T00:00:00Z".into(),
        }
    }

    fn output(directory: &Path, artifact: Option<&Path>) -> RunOutput {
        RunOutput {
            workspace_id: "ws".into(),
            workspace_root: directory.to_string_lossy().to_string(),
            output_directory: Some(directory.to_string_lossy().to_string()),
            task_id: None,
            run_id: "run-1".into(),
            title: "Teste".into(),
            summary: String::new(),
            artifacts: artifact
                .map(|path| {
                    vec![RunArtifact {
                        file_path: "dados.json".into(),
                        absolute_path: path.to_string_lossy().to_string(),
                        action_type: "CREATED".into(),
                    }]
                })
                .unwrap_or_default(),
        }
    }

    #[tokio::test]
    async fn inserts_every_json_row() {
        let ctx = crate::test_support::context().await;
        let directory = ctx.temp.path().join("sink");
        std::fs::create_dir_all(&directory).expect("dir");
        let artifact = directory.join("dados.json");
        std::fs::write(
            &artifact,
            json!([
                { "name": "alpha", "count": 2, "ok": true },
                { "name": "beta", "count": null, "ok": false }
            ])
            .to_string(),
        )
        .expect("json");
        let database = directory.join("destino.db");
        let url = format!("sqlite://{}?mode=rwc", database.to_string_lossy());
        sqlx::any::install_default_drivers();
        let pool = crate::io_bus::sql::connect(&url).await.expect("connect");
        sqlx::query("CREATE TABLE registros (name TEXT, count INTEGER, ok BOOLEAN)")
            .execute(&pool)
            .await
            .expect("table");
        pool.close().await;

        let io = IoContext {
            pool: ctx.state.db.clone(),
            paths: ctx.state.paths.clone(),
        };
        ctx.state
            .secrets
            .set("io_bus", "test-sql-sink-url", &url)
            .expect("secret");
        let result = run(
            &io,
            &sink(json!({
                "database_url_ref": "test-sql-sink-url",
                "query": "INSERT INTO registros (name, count, ok) VALUES (?, ?, ?)",
                "columns": ["name", "count", "ok"]
            })),
            &output(&directory, Some(&artifact)),
        )
        .await
        .expect("sink");
        assert_eq!(result, vec!["2 linha(s) afetada(s)"]);

        let pool = crate::io_bus::sql::connect(&format!(
            "sqlite://{}?mode=rwc",
            database.to_string_lossy()
        ))
        .await
        .expect("reconnect");
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM registros")
            .fetch_one(&pool)
            .await
            .expect("count");
        assert_eq!(total, 2);
        pool.close().await;
    }

    #[tokio::test]
    async fn explains_missing_or_invalid_artifacts() {
        let ctx = crate::test_support::context().await;
        let directory = ctx.temp.path().join("sink-errors");
        std::fs::create_dir_all(&directory).expect("dir");
        let io = IoContext {
            pool: ctx.state.db.clone(),
            paths: ctx.state.paths.clone(),
        };
        let database = format!("sqlite://{}", directory.join("erros.db").to_string_lossy());
        ctx.state
            .secrets
            .set("io_bus", "test-sql-sink-errors", &database)
            .expect("secret");
        let config = json!({
            "database_url_ref": "test-sql-sink-errors",
            "query": "INSERT INTO x VALUES (?)",
            "columns": ["name"]
        });

        let missing = run(&io, &sink(config.clone()), &output(&directory, None))
            .await
            .expect_err("missing");
        assert!(format!("{missing}").contains("artefato JSON"));

        let invalid = directory.join("invalido.json");
        std::fs::write(&invalid, "not json").expect("json");
        let error = run(&io, &sink(config), &output(&directory, Some(&invalid)))
            .await
            .expect_err("invalid");
        assert!(format!("{error}").contains("JSON inválido"));
    }
}
