use crate::{
    app_state::AppState,
    db::{
        self,
        io_bus::{InboundSource, OutboundSink},
        tasks::{self, CreateTaskInput},
    },
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
    paths::AppPaths,
    secrets::SecretStore,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

pub mod chat;
pub mod inbound;
pub mod outbound;
pub mod preprocess;
pub mod runtime;
pub mod sql;
pub mod tools;

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

/// Convert any inbound signal into exactly one deterministic task.
pub async fn ingest(
    state: &Arc<AppState>,
    source: &InboundSource,
    event: NormalizedTaskEvent,
) -> AppResult<Option<tasks::Task>> {
    // A routine can subscribe to this source. The routine decides whether it
    // runs a prompt or a recipe, so ingest does not create a task in that case.
    if let Ok(config) = serde_json::from_str::<serde_json::Value>(&source.config_json)
        && let Some(schedule_id) = config.get("schedule_id").and_then(|value| value.as_str())
        && let Ok(schedule) = crate::db::schedules::get(&state.db, schedule_id).await
    {
        return crate::api::schedules::execute_schedule_from_source(state, &schedule, &event).await;
    }
    // Sources can route their signal to a declarative recipe instead of creating
    // a standalone task. This is how IMAP/Signal/SQL/webhook triggers reuse the
    // recipe engine without duplicating adapters.
    if let Ok(config) = serde_json::from_str::<serde_json::Value>(&source.config_json)
        && let Some(recipe_id) = config.get("recipe_id").and_then(|value| value.as_str())
    {
        let yaml =
            crate::recipes::store::yaml_for(&state.db, &source.workspace_id, recipe_id).await?;
        let recipe = crate::recipes::parser::parse(&yaml)?;
        let trigger = serde_json::json!({
            "source_id": source.id,
            "external_id": event.external_id,
            "title": event.title,
            "body": event.body_markdown,
            "attachments": event.attachments,
            "metadata": event.metadata,
        });
        let run = crate::recipes::engine::start(
            crate::app_state::recipe_context(state),
            &source.workspace_id,
            recipe,
            trigger,
            false,
        )
        .await?;
        if let Some(task_id) = run.task_id {
            return Ok(Some(tasks::get(&state.db, &task_id).await?));
        }
        return Err(AppError::Internal(
            "recipe acionada por I/O não criou task".into(),
        ));
    }
    let seed = format!(
        "opendesk:io:{}:{}",
        source.id,
        if event.external_id.trim().is_empty() {
            format!("{}:{}", event.title, event.body_markdown)
        } else {
            event.external_id.clone()
        }
    );
    let id = Uuid::new_v5(&Uuid::NAMESPACE_URL, seed.as_bytes()).to_string();
    if let Ok(existing) = tasks::get(&state.db, &id).await {
        return Ok(Some(existing));
    }

    let mut body = event.body_markdown.trim().to_string();
    if body.is_empty() {
        body = event.title.clone();
    }
    if !event.attachments.is_empty() {
        body.push_str("\n\n## Arquivos de entrada\n\n");
        for path in &event.attachments {
            body.push_str(&format!("- `{path}`\n"));
        }
    }
    let mut tags = event.tags.clone();
    tags.push("io-bus".into());
    tags.push(format!("io-source:{}", source.id));
    let task = tasks::create_with_id(
        &state.db,
        &source.workspace_id,
        Some(&id),
        CreateTaskInput {
            title: event.title.chars().take(180).collect(),
            description: body,
            execution_mode: Some("AUTO_DSH".into()),
            priority: event.priority.or(Some(2)),
            assigned_agent: source.target_agent.clone(),
            preferred_model: None,
            preferred_harness: None,
            effort_level: None,
            care_level: Some("standard".into()),
            notify_email: None,
            export_to_gdocs: false,
            tags,
            origin: Some("IO_BUS".into()),
            parent_task_id: None,
            dedupe: None,
        },
    )
    .await?;
    if task.deduplicated {
        let _ = db::io_bus::mark_inbound(&state.db, &source.id, "DEDUPLICATED", None).await;
        return Ok(Some(task));
    }

    if source.target_agent.is_some() {
        let _ = tasks::update(
            &state.db,
            &task.id,
            tasks::UpdateTaskInput {
                agent_override: Some(true),
                ..Default::default()
            },
        )
        .await;
    }
    if !source.auto_execute {
        let column = if source.target_column.is_empty() {
            "TODO"
        } else {
            &source.target_column
        };
        let _ = tasks::move_to(&state.db, &task.id, column, None).await;
    }
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: task.status.clone(),
        phase_status: task.phase_status.clone(),
    });

    if source.auto_execute {
        let body = crate::api::tasks::DispatchBody {
            policy: None,
            manual_agent: source.target_agent.clone(),
            manual_model: None,
            manual_effort: None,
            execution_mode: None,
            prompt_append: None,
        };
        if let Err(error) =
            crate::api::tasks::dispatch_task_internal(state, &task.id, body, None).await
        {
            tracing::warn!(task = %task.id, error = %error, "io bus task queued without engine dispatch");
            let _ = db::io_bus::mark_inbound(
                &state.db,
                &source.id,
                "QUEUED_TASK",
                Some(&format!("{error:#}")),
            )
            .await;
            let column = if source.target_column.is_empty() {
                "TODO"
            } else {
                &source.target_column
            };
            let _ = tasks::move_to(&state.db, &task.id, column, None).await;
            return Ok(Some(tasks::get(&state.db, &task.id).await?));
        }
    } else {
        let _ = db::io_bus::mark_inbound(&state.db, &source.id, "TASK_CREATED", None).await;
    }
    Ok(Some(tasks::get(&state.db, &task.id).await?))
}

#[derive(Debug, Clone, Default)]
pub struct RunOutput {
    pub workspace_id: String,
    pub workspace_root: String,
    pub output_directory: Option<String>,
    pub task_id: Option<String>,
    pub run_id: String,
    pub title: String,
    pub summary: String,
    pub artifacts: Vec<RunArtifact>,
}

#[derive(Debug, Clone)]
pub struct RunArtifact {
    pub file_path: String,
    pub absolute_path: String,
    pub action_type: String,
}

pub async fn load_run_output(
    ctx: &IoContext,
    workspace_id: &str,
    task_id: Option<&str>,
    _schedule_id: Option<&str>,
    run_id: &str,
) -> AppResult<RunOutput> {
    let run = db::runs::get(&ctx.pool, run_id).await?;
    let workspace = db::workspaces::get(&ctx.pool, workspace_id).await?;
    let output_directory = if let Some(task_id) = task_id {
        db::tasks::get(&ctx.pool, task_id)
            .await
            .ok()
            .and_then(|task| task.output_dir_path)
    } else {
        None
    };
    let artifacts = db::artifacts::list_by_run(&ctx.pool, run_id, 500)
        .await?
        .into_iter()
        .map(|artifact| RunArtifact {
            file_path: artifact.file_path,
            absolute_path: artifact.absolute_path,
            action_type: artifact.action_type,
        })
        .collect();
    Ok(RunOutput {
        workspace_id: workspace_id.to_string(),
        workspace_root: workspace.root_path,
        output_directory,
        task_id: task_id.map(str::to_string),
        run_id: run_id.to_string(),
        title: run
            .task_title
            .clone()
            .unwrap_or_else(|| "OpenDesk run".into()),
        summary: run.summary_response.unwrap_or_default(),
        artifacts,
    })
}

/// Execute all sinks bound to a task/schedule after a successful run. Sink
/// failures are isolated and never change the run status.
pub async fn run_sinks(
    ctx: &IoContext,
    workspace_id: &str,
    task_id: Option<&str>,
    schedule_id: Option<&str>,
    run_id: &str,
) -> AppResult<()> {
    let sinks = db::io_bus::sinks_for_run(&ctx.pool, task_id, schedule_id).await?;
    if sinks.is_empty() {
        return Ok(());
    }
    let output = load_run_output(ctx, workspace_id, task_id, schedule_id, run_id).await?;
    for sink in sinks {
        let result = outbound::execute(ctx, &sink, &output).await;
        match &result {
            Ok(_) => {
                let _ = db::io_bus::mark_outbound(&ctx.pool, &sink.id, "SUCCESS", None).await;
            }
            Err(error) => {
                tracing::warn!(sink = %sink.id, run = %run_id, error = %error, "outbound sink failed");
                let _ = db::io_bus::mark_outbound(
                    &ctx.pool,
                    &sink.id,
                    "FAILED",
                    Some(&format!("{error:#}")),
                )
                .await;
            }
        }
    }
    Ok(())
}

/// Resolve a secret from SecretStore or an explicit plain value. Used by every
/// IO bus adapter so credential rules stay in one place.
pub fn secret(paths: &AppPaths, reference: Option<&str>, plain: Option<&str>) -> AppResult<String> {
    if let Some(value) = plain.filter(|value| !value.trim().is_empty()) {
        return Ok(value.to_string());
    }
    let key =
        reference.ok_or_else(|| AppError::BadRequest("referência de segredo ausente".into()))?;
    SecretStore::new(paths)
        .get("io_bus", key)?
        .ok_or_else(|| AppError::NotFound(format!("segredo {key}")))
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

pub const WEBHOOK_SECRET_NAMESPACE: &str = "io_bus";

/// Moves a plaintext webhook secret into the encrypted store and guarantees a
/// `secret_ref` in the config. Every `WEBHOOK_HTTP` source must carry one, or a
/// request could create tasks and start runs without authentication.
///
/// The plain value is removed from the config so it never reaches the database,
/// backups or logs.
pub fn ensure_webhook_secret(
    paths: &AppPaths,
    config: &mut serde_json::Value,
) -> AppResult<String> {
    let reference = format!("webhook-{}", Uuid::now_v7());
    ensure_webhook_secret_with(paths, config, &reference)
}

/// Deterministic variant used by routines and recipes so saving the same form
/// twice overwrites the same stored secret instead of leaving orphans.
pub fn ensure_webhook_secret_with(
    paths: &AppPaths,
    config: &mut serde_json::Value,
    fallback_reference: &str,
) -> AppResult<String> {
    let Some(object) = config.as_object_mut() else {
        return Err(AppError::BadRequest(
            "config do webhook deve ser um objeto JSON".into(),
        ));
    };
    if let Some(reference) = object
        .get("secret_ref")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(reference.to_string());
    }
    let plain = object
        .get("secret")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            AppError::BadRequest(
                "webhook HTTP exige segredo; defina o segredo do webhook na rotina ou receita"
                    .into(),
            )
        })?;
    SecretStore::new(paths).set(WEBHOOK_SECRET_NAMESPACE, fallback_reference, &plain)?;
    object.remove("secret");
    object.insert(
        "secret_ref".into(),
        serde_json::Value::String(fallback_reference.to_string()),
    );
    Ok(fallback_reference.to_string())
}

/// Removes a secret that is no longer referenced by any source. Best effort.
pub fn drop_webhook_secret(paths: &AppPaths, reference: &str) {
    if reference.trim().is_empty() {
        return;
    }
    if let Err(error) = SecretStore::new(paths).delete(WEBHOOK_SECRET_NAMESPACE, reference.trim()) {
        tracing::warn!(error = %error, "could not delete webhook secret");
    }
}

pub fn webhook_secret_ref(config_json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(config_json)
        .ok()
        .and_then(|config| {
            config
                .get("secret_ref")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::io_bus::{InboundSource, OutboundSink};

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
