use crate::{
    db::io_bus::{InboundSource, OutboundSink},
    error::{AppError, AppResult},
    paths::AppPaths,
    secrets::SecretStore,
};
use serde::{Deserialize, Serialize};

pub mod chat;
pub mod inbound;
mod ingestion;
pub mod outbound;
mod output;
pub mod preprocess;
pub mod runtime;
mod secrets;
pub mod secrets_guard;
pub mod sql;
pub mod ssrf;
pub mod tools;

pub use ingestion::ingest;
#[allow(unused_imports)]
pub use output::load_run_output;
pub use output::{RunArtifact, RunOutput, run_sinks};
pub use secrets::{
    WEBHOOK_SECRET_NAMESPACE, drop_webhook_secret, ensure_webhook_secret,
    ensure_webhook_secret_with, secret, webhook_secret_ref,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedTaskEvent {
    pub source_id: String,
    pub external_id: String,
    pub title: String,
    pub body_markdown: String,
    #[serde(default)]
    pub priority: Option<i64>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub attachments: Vec<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Clone)]
pub struct IoContext {
    pub pool: sqlx::SqlitePool,
    pub paths: AppPaths,
}

impl IoContext {
    pub fn secrets(&self) -> SecretStore {
        SecretStore::new(&self.paths)
    }
}

pub fn http_client(timeout_secs: u64) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
        .unwrap_or_default()
}

pub fn config_value<T: for<'de> Deserialize<'de>>(sink: &OutboundSink) -> AppResult<T> {
    serde_json::from_str(&sink.config_json)
        .map_err(|error| AppError::BadRequest(format!("config_json inválido: {error}")))
}

pub fn source_config<T: for<'de> Deserialize<'de>>(source: &InboundSource) -> AppResult<T> {
    serde_json::from_str(&source.config_json)
        .map_err(|error| AppError::BadRequest(format!("config_json inválido: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::io_bus::{InboundSource, OutboundSink};
    use std::sync::Arc;

    #[tokio::test]
    async fn ingests_same_external_id_as_one_task() {
        use crate::paths::AppPaths;
        let temp = tempfile::TempDir::new().expect("temp");
        let root = temp.path();
        let paths = AppPaths {
            data_dir: root.join("data"),
            config_dir: root.join("config"),
            cache_dir: root.join("cache"),
            log_dir: root.join("logs"),
            db_path: root.join("data").join("opendesk.db"),
            backups_dir: root.join("backups"),
            artifacts_dir: root.join("artifacts"),
            run_logs_dir: root.join("runs"),
            daemon_lock_path: root.join("daemon.json"),
            api_lock_path: root.join("api.json"),
            allow_temp_workspaces: true,
        };
        for dir in [
            &paths.data_dir,
            &paths.config_dir,
            &paths.cache_dir,
            &paths.log_dir,
            &paths.backups_dir,
            &paths.artifacts_dir,
            &paths.run_logs_dir,
        ] {
            std::fs::create_dir_all(dir).expect("dir");
        }
        let state = Arc::new(
            crate::app_state::AppState::initialize(paths)
                .await
                .expect("state"),
        );
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(root.to_string_lossy().to_string())
            .execute(&state.db)
            .await
            .expect("workspace");
        crate::secrets::SecretStore::new(&state.paths)
            .set("io_bus", "wh-test", "segredo-de-teste")
            .expect("secret");
        let source = crate::db::io_bus::create_inbound(
            &state.db,
            "ws-1",
            crate::db::io_bus::InboundInput {
                name: "webhook".into(),
                source_type: "WEBHOOK_HTTP".into(),
                config: serde_json::json!({ "secret_ref": "wh-test" }),
                target_agent: None,
                target_column: Some("TODO".into()),
                target_kind: None,
                target_id: None,
                auto_execute: false,
                is_enabled: true,
            },
        )
        .await
        .expect("source");
        let event = NormalizedTaskEvent {
            source_id: source.id.clone(),
            external_id: "same".into(),
            title: "Tarefa".into(),
            body_markdown: "corpo".into(),
            priority: None,
            tags: vec![],
            attachments: vec![],
            metadata: serde_json::json!({}),
        };
        let first = ingest(&state, &source, event.clone())
            .await
            .expect("first")
            .expect("task");
        let second = ingest(&state, &source, event)
            .await
            .expect("second")
            .expect("task");
        assert_eq!(first.id, second.id);
        assert_eq!(first.status, "TODO");
    }

    #[test]
    fn parses_config_values_and_rejects_invalid_json() {
        let sink = OutboundSink {
            id: "s".into(),
            workspace_id: "ws".into(),
            name: "S".into(),
            sink_type: "DOC_COMPILER".into(),
            config_json: "{\"format\":\"pdf\"}".into(),
            is_enabled: true,
            last_run_at: None,
            last_status: None,
            last_error: None,
            created_at: "now".into(),
        };
        let value: serde_json::Value = config_value(&sink).expect("config");
        assert_eq!(value["format"], "pdf");
        let source = InboundSource {
            id: "i".into(),
            workspace_id: "ws".into(),
            name: "I".into(),
            source_type: "FOLDER_WATCH".into(),
            config_json: "{invalid".into(),
            target_agent: None,
            target_column: "TODO".into(),
            target_kind: "TASK".into(),
            target_id: None,
            auto_execute: true,
            is_enabled: true,
            last_triggered_at: None,
            last_status: None,
            last_error: None,
            created_at: "now".into(),
        };
        assert!(source_config::<serde_json::Value>(&source).is_err());
    }
}
