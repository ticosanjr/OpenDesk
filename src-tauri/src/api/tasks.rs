use crate::{
    app_state::AppState,
    db::{
        self,
        queue::{self, EnqueueInput},
        runs, settings,
        task_messages::{self, CreateTaskMessageInput},
        tasks::{self, CreateTaskInput, UpdateTaskInput},
        workspaces,
    },
    dsh_router::{self, PackContext},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
    opencode_client::rest_client::ModelRefDto,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

mod task_dispatch;

use task_dispatch::{base_model, concurrency_group, enqueue_decision, model_key, plan_decision};
pub(crate) use task_dispatch::{dispatch_task_internal, dispatch_task_with_session};

#[derive(Debug, Deserialize, Default)]
pub struct TaskQuery {
    pub workspace_id: Option<String>,
    /// `work` hides routes/recipes/IO tasks from the Kanban.
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct DispatchBody {
    pub policy: Option<String>,
    pub manual_agent: Option<String>,
    pub manual_model: Option<String>,
    pub manual_effort: Option<String>,
    pub execution_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MoveBody {
    #[serde(alias = "column_stage")]
    pub status: String,
    pub position_index: Option<i64>,
}

pub async fn list_tasks(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TaskQuery>,
) -> AppResult<Json<Vec<tasks::Task>>> {
    let workspace_id = resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let list = if query.scope.as_deref() == Some("work") {
        tasks::list_work(&state.db, &workspace_id).await?
    } else {
        tasks::list(&state.db, &workspace_id).await?
    };
    Ok(Json(list))
}

pub(super) fn validate_model_ref(value: Option<&str>) -> AppResult<()> {
    if let Some(value) = value.map(str::trim).filter(|value| !value.is_empty())
        && ModelRefDto::parse(value).is_none()
    {
        return Err(AppError::BadRequest(format!(
            "modelo inválido '{value}'; use provider/modelo ou provider/modelo#variant"
        )));
    }
    Ok(())
}

pub async fn create_task(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TaskQuery>,
    Json(input): Json<CreateTaskInput>,
) -> AppResult<Json<tasks::Task>> {
    validate_model_ref(input.preferred_model.as_deref())?;
    let workspace_id = resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let task = tasks::create(&state.db, &workspace_id, input).await?;
    let task = if task.deduplicated {
        task
    } else {
        try_autostart(&state, &task).await.unwrap_or(task)
    };
    Ok(Json(task))
}

pub async fn get_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<tasks::Task>> {
    Ok(Json(tasks::get(&state.db, &id).await?))
}

pub async fn update_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<UpdateTaskInput>,
) -> AppResult<Json<tasks::Task>> {
    validate_model_ref(input.preferred_model.as_deref())?;
    let task = tasks::update(&state.db, &id, input).await?;
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: task.status.clone(),
        phase_status: task.phase_status.clone(),
    });
    Ok(Json(task))
}

pub async fn delete_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let task = tasks::get(&state.db, &id).await?;
    // A queue job keeps its own prompt payload, so deleting the task alone
    // would leave an orphan run. Cancel active jobs first.
    let jobs = queue::list(&state.db, Some(&task.workspace_id)).await?;
    for job in jobs
        .into_iter()
        .filter(|job| job.task_id.as_deref() == Some(task.id.as_str()))
        .filter(|job| matches!(job.status.as_str(), "RUNNING" | "QUEUED" | "PAUSED"))
    {
        let _ = state.queue.cancel(&job.id).await;
    }
    tasks::delete(&state.db, &id).await?;
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: "DELETED".into(),
        phase_status: None,
    });
    Ok(StatusCode::NO_CONTENT)
}

pub async fn move_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<MoveBody>,
) -> AppResult<Json<tasks::Task>> {
    let task = tasks::move_to(&state.db, &id, &body.status, body.position_index).await?;
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: task.status.clone(),
        phase_status: task.phase_status.clone(),
    });
    Ok(Json(task))
}

pub async fn dispatch_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<DispatchBody>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(dispatch_task_internal(&state, &id, body, None).await?))
}

/// Shared dispatch path used by HTTP handlers, cron and the I/O bus. It keeps
/// router, budget, queue and status transitions in one place.
pub async fn approve_plan(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<DispatchBody>,
) -> AppResult<Json<serde_json::Value>> {
    validate_model_ref(body.manual_model.as_deref())?;
    crate::api::engine::require_engine(&state).await?;
    let task = tasks::get(&state.db, &id).await?;
    let plan = task
        .plan_markdown
        .clone()
        .ok_or_else(|| AppError::BadRequest("this task has no plan to approve".into()))?;
    let workspace = workspaces::get(&state.db, &task.workspace_id).await?;
    let decision = plan_decision(&state, &task, &workspace, &body, Some(&plan)).await?;
    let job = enqueue_decision(&state, &task, &workspace, &decision, Some(&plan)).await?;
    Ok(Json(serde_json::json!({
        "decision": decision,
        "job": job,
    })))
}

pub async fn resolve_workspace_id(state: &AppState, requested: Option<&str>) -> AppResult<String> {
    if let Some(id) = requested {
        return Ok(id.to_string());
    }
    db::settings::active_workspace_id(&state.db)
        .await?
        .ok_or_else(|| AppError::BadRequest("no active workspace".into()))
}

pub async fn fork_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Option<Json<ForkBody>>,
) -> AppResult<Json<tasks::Task>> {
    let task = tasks::fork(&state.db, &id).await?;
    let task = apply_fork_overrides(&state, task, body.map(|Json(body)| body)).await?;
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: task.status.clone(),
        phase_status: task.phase_status.clone(),
    });
    Ok(Json(task))
}

/// Retry a failed run on a different, healthy model. Used by the Attention
/// Card so a provider outage does not force the user into the full board.
pub async fn retry_other_model(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    crate::api::engine::require_engine_strict(&state).await?;
    let run = runs::get(&state.db, &run_id).await?;
    let task_id = run
        .task_id
        .clone()
        .ok_or_else(|| AppError::BadRequest("run sem tarefa associada".into()))?;
    let task = tasks::get(&state.db, &task_id).await?;
    let workspace = workspaces::get(&state.db, &task.workspace_id).await?;
    let failed = base_model(&run.model_id);
    let cooling = db::model_health::cooling(&state.db, 24)
        .await
        .unwrap_or_default();
    let models = state
        .daemon
        .models(Some(&workspace.root_path))
        .await
        .unwrap_or_default();
    let fallback = models
        .iter()
        .filter(|model| model.enabled.unwrap_or(true))
        .map(model_key)
        .find(|model| {
            let base = base_model(model);
            base != failed && !cooling.contains(&base)
        })
        .ok_or_else(|| AppError::BadRequest("nenhum outro modelo saudável disponível".into()))?;

    let decision = plan_decision(
        &state,
        &task,
        &workspace,
        &DispatchBody {
            policy: None,
            manual_agent: None,
            manual_model: Some(fallback.clone()),
            manual_effort: None,
            execution_mode: None,
        },
        None,
    )
    .await?;
    let job = enqueue_decision(&state, &task, &workspace, &decision, None).await?;
    Ok(Json(serde_json::json!({
        "model": fallback,
        "job": job,
    })))
}

#[derive(Debug, Deserialize)]
pub struct SwarmBody {
    pub models: Vec<String>,
}

/// Run the same task on two or more models in isolated output folders so the
/// user can compare the deliverables and keep the winner.
pub async fn swarm_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<SwarmBody>,
) -> AppResult<Json<serde_json::Value>> {
    crate::api::engine::require_engine_strict(&state).await?;
    let models = body
        .models
        .iter()
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
        .collect::<Vec<_>>();
    if !(2..=4).contains(&models.len()) {
        return Err(AppError::BadRequest(
            "escolha de 2 a 4 modelos para o swarm".into(),
        ));
    }
    for model in &models {
        validate_model_ref(Some(model))?;
    }

    let source = tasks::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &source.workspace_id).await?;
    let group = Uuid::now_v7().to_string();
    let mut created = Vec::new();

    for model in models {
        let forked = tasks::fork_with_model(&state.db, &source.id, Some(&model)).await?;
        let directory =
            dsh_router::output_directory(std::path::Path::new(&workspace.root_path), &forked.title);
        tasks::set_output_dir(&state.db, &forked.id, &directory.to_string_lossy()).await?;
        let forked = tasks::get(&state.db, &forked.id).await?;
        let decision = plan_decision(
            &state,
            &forked,
            &workspace,
            &DispatchBody {
                policy: None,
                manual_agent: None,
                manual_model: Some(model.clone()),
                manual_effort: None,
                execution_mode: Some(forked.execution_mode.clone()),
            },
            None,
        )
        .await?;
        enqueue_decision(&state, &forked, &workspace, &decision, None).await?;
        created.push(serde_json::json!({
            "id": forked.id,
            "title": forked.title,
            "model": model,
            "output_dir": directory.to_string_lossy(),
        }));
    }

    Ok(Json(serde_json::json!({
        "group": group,
        "tasks": created,
    })))
}

/// Fork a task preserving the deliverables produced up to a specific run.
/// Files are copied into a fresh output folder so the original stays intact.
pub async fn fork_from_run(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
    body: Option<Json<ForkBody>>,
) -> AppResult<Json<tasks::Task>> {
    let run = runs::get(&state.db, &run_id).await?;
    let task_id = run
        .task_id
        .clone()
        .ok_or_else(|| AppError::BadRequest("run sem tarefa associada".into()))?;
    let source = tasks::get(&state.db, &task_id).await?;
    let workspace = workspaces::get(&state.db, &source.workspace_id).await?;
    let forked = tasks::fork_with_model(&state.db, &source.id, None).await?;
    let forked = apply_fork_overrides(&state, forked, body.map(|Json(body)| body)).await?;
    let output =
        dsh_router::output_directory(std::path::Path::new(&workspace.root_path), &forked.title);
    std::fs::create_dir_all(&output)?;

    let cutoff = run
        .completed_at
        .clone()
        .unwrap_or_else(|| run.started_at.clone());
    let artifacts = db::artifacts::list(
        &state.db,
        &source.workspace_id,
        None,
        Some(&source.id),
        None,
        500,
    )
    .await
    .unwrap_or_default();
    let mut copied = Vec::new();
    for artifact in artifacts.iter().filter(|item| item.created_at <= cutoff) {
        let source_file = std::path::Path::new(&artifact.absolute_path);
        if !source_file.is_file() {
            continue;
        }
        let Some(name) = source_file.file_name() else {
            continue;
        };
        let mut destination = output.join(name);
        if destination.exists() {
            let stem = source_file
                .file_stem()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| "artifact".into());
            let extension = source_file
                .extension()
                .map(|value| format!(".{}", value.to_string_lossy()))
                .unwrap_or_default();
            destination = output.join(format!("{stem}-{}{extension}", artifact.id));
        }
        if std::fs::copy(source_file, &destination).is_ok() {
            copied.push(
                destination
                    .strip_prefix(&workspace.root_path)
                    .unwrap_or(&destination)
                    .to_string_lossy()
                    .to_string(),
            );
        }
    }

    let mut description = forked.description.clone();
    description.push_str(&format!(
        "\n\n## Fork a partir de {}\n\n{} arquivo(s) herdados em `{}`.",
        run_id,
        copied.len(),
        output
            .strip_prefix(&workspace.root_path)
            .unwrap_or(&output)
            .to_string_lossy()
    ));
    for path in &copied {
        description.push_str(&format!("\n- {path}"));
    }
    tasks::set_output_dir(&state.db, &forked.id, &output.to_string_lossy()).await?;
    let forked = tasks::update(
        &state.db,
        &forked.id,
        UpdateTaskInput {
            description: Some(description),
            ..Default::default()
        },
    )
    .await?;
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: forked.id.clone(),
        workspace_id: forked.workspace_id.clone(),
        stage: forked.status.clone(),
        phase_status: forked.phase_status.clone(),
    });
    Ok(Json(forked))
}

pub async fn get_task_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let task = tasks::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &task.workspace_id).await?;
    let path = std::path::Path::new(&workspace.root_path)
        .join(".opencode")
        .join("MEMORY.md");
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    Ok(Json(serde_json::json!({ "content": content })))
}

#[derive(Debug, Deserialize)]
pub struct MemoryBody {
    pub content: String,
    #[serde(default)]
    pub run_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct MemoryQuery {
    pub run_id: Option<String>,
}

fn memory_marker(task_id: &str, run_id: Option<&str>) -> String {
    format!(
        "<!-- opendesk:memory task:{task_id} run:{} -->",
        run_id.unwrap_or("-")
    )
}

fn memory_block(marker: &str, content: &str) -> String {
    format!(
        "\n{marker}\n- [{}] {}\n<!-- /opendesk:memory -->\n",
        chrono::Utc::now().format("%Y-%m-%d"),
        content.replace('\n', " ")
    )
}

fn remove_memory_blocks(content: &str, task_id: &str, run_id: Option<&str>) -> String {
    let prefix = format!("<!-- opendesk:memory task:{task_id}");
    let wanted = run_id.map(|run_id| memory_marker(task_id, Some(run_id)));
    let mut output = String::new();
    let mut skipping = false;
    for line in content.lines() {
        if line.starts_with(&prefix) {
            let matches = wanted.as_deref().is_none_or(|marker| line.trim() == marker);
            skipping = matches;
            if !matches {
                output.push_str(line);
                output.push('\n');
            }
            continue;
        }
        if skipping {
            if line.trim() == "<!-- /opendesk:memory -->" {
                skipping = false;
            }
            continue;
        }
        output.push_str(line);
        output.push('\n');
    }
    output.trim_end().to_string() + if output.trim().is_empty() { "" } else { "\n" }
}

pub async fn post_task_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<MemoryBody>,
) -> AppResult<Json<serde_json::Value>> {
    let content = body.content.trim();
    if content.is_empty() {
        return Err(AppError::BadRequest("memory content is required".into()));
    }
    let task = tasks::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &task.workspace_id).await?;
    let dir = std::path::Path::new(&workspace.root_path).join(".opencode");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("MEMORY.md");
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let cleaned = remove_memory_blocks(&existing, &id, body.run_id.as_deref());
    let marker = memory_marker(&id, body.run_id.as_deref());
    let next = format!("{}{}", cleaned.trim_end(), memory_block(&marker, content));
    crate::config_sync::file_ops::write_text_atomic(&path, &next, None, &state.paths.backups_dir)?;
    Ok(Json(serde_json::json!({ "ok": true, "path": path })))
}

pub async fn delete_task_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<MemoryQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let task = tasks::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &task.workspace_id).await?;
    let path = std::path::Path::new(&workspace.root_path)
        .join(".opencode")
        .join("MEMORY.md");
    if !path.exists() {
        return Ok(Json(serde_json::json!({ "ok": true, "removed": false })));
    }
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let next = remove_memory_blocks(&existing, &id, query.run_id.as_deref());
    if next != existing {
        crate::config_sync::file_ops::write_text_atomic(
            &path,
            &next,
            None,
            &state.paths.backups_dir,
        )?;
    }
    Ok(Json(
        serde_json::json!({ "ok": true, "removed": next != existing }),
    ))
}

pub async fn list_task_messages(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<task_messages::TaskMessage>>> {
    tasks::get(&state.db, &id).await?;
    Ok(Json(task_messages::list(&state.db, &id).await?))
}

pub async fn post_task_message(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<CreateTaskMessageInput>,
) -> AppResult<Json<serde_json::Value>> {
    validate_model_ref(input.model.as_deref())?;
    let task = tasks::get(&state.db, &id).await?;
    let message = task_messages::create(
        &state.db,
        &id,
        "user",
        "comment",
        &input.content,
        input.run_id.as_deref(),
        None,
    )
    .await?;

    if !input.continue_execution {
        return Ok(Json(serde_json::json!({ "message": message, "job": null })));
    }

    crate::api::engine::require_engine_strict(&state).await?;
    let workspace = workspaces::get(&state.db, &task.workspace_id).await?;
    let mut decision = plan_decision(
        &state,
        &task,
        &workspace,
        &DispatchBody {
            manual_agent: input.agent.clone(),
            manual_model: input.model.clone(),
            ..Default::default()
        },
        task.plan_markdown.as_deref(),
    )
    .await?;
    decision.phase = "follow_up".into();

    std::fs::create_dir_all(&decision.output_directory).map_err(|error| {
        AppError::Internal(format!("could not create output directory: {error}"))
    })?;
    tasks::set_output_dir(&state.db, &task.id, &decision.output_directory).await?;

    let history = task_messages::list(&state.db, &id).await?;
    let conversation = history
        .iter()
        .filter(|item| item.role == "user")
        .map(|item| format!("Usuário: {}", item.content))
        .collect::<Vec<_>>()
        .join(
            "

",
        );
    let tags = task.tags();
    let needs_browser = crate::browse::ensure_for_task(
        &state,
        &task.workspace_id,
        &task.title,
        &task.description,
        &tags,
    )
    .await?;
    let packed = dsh_router::pack(&PackContext {
        workspace_root: std::path::Path::new(&workspace.root_path),
        output_directory: &decision.output_directory,
        task_title: &task.title,
        task_description: &task.description,
        tags: &tags,
        priority: task.priority,
        phase: "follow_up",
        plan: task.plan_markdown.as_deref(),
        agent: &decision.selected_agent,
        needs_browser,
    });
    let prompt = format!(
        "{packed}\n\n## Conversa com o usuário\n\n{conversation}\n\nContinue a execução considerando a mensagem acima. Task ID: `{}`. Se precisar de uma decisão, use a ferramenta MCP `ask_user` com este task_id antes de assumir.",
        task.id
    );

    let session_id = runs::latest_session_for_task(&state.db, &task.id).await?;
    let job = state
        .queue
        .enqueue(EnqueueInput {
            workspace_id: task.workspace_id.clone(),
            task_id: Some(task.id.clone()),
            schedule_id: None,
            session_id,
            concurrency_group: Some(concurrency_group(&decision.selected_agent)),
            prompt_payload: prompt,
            resolved_mode: "build".into(),
            resolved_agent: decision.selected_agent.clone(),
            resolved_model: decision.selected_model.clone(),
            resolved_effort: decision.effort.clone(),
            job_kind: "follow_up".into(),
            scheduled_for: None,
            output_dir: Some(decision.output_directory.clone()),
            priority: task.priority,
            max_retries: 1,
        })
        .await?;

    tasks::move_to(&state.db, &task.id, "TODO", None).await?;
    tasks::set_phase(&state.db, &task.id, Some("follow_up")).await?;
    tasks::set_phase_status(&state.db, &task.id, Some("follow_up_queued"), Some("TODO")).await?;
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: "TODO".into(),
        phase_status: Some("follow_up_queued".into()),
    });

    Ok(Json(serde_json::json!({ "message": message, "job": job })))
}

pub async fn task_diagnostics(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let task = tasks::get(&state.db, &id).await?;
    let jobs = queue::list(&state.db, Some(&task.workspace_id)).await?;
    let jobs = jobs
        .into_iter()
        .filter(|job| job.task_id.as_deref() == Some(task.id.as_str()))
        .collect::<Vec<_>>();
    let runs = runs::list(&state.db, &task.workspace_id, Some(&task.id)).await?;
    let engine = state.daemon.status().await;

    let failed_run = runs
        .iter()
        .find(|run| run.status == "FAILED" || run.status == "CANCELLED");
    let failed_job = jobs
        .iter()
        .find(|job| matches!(job.status.as_str(), "FAILED" | "CANCELLED"));

    let error = failed_run
        .and_then(|run| {
            let message = run.error_message.clone().or_else(|| run.summary_response.clone())?;
            Some(serde_json::json!({
                "code": run.error_code.clone().unwrap_or_else(|| "run_failed".into()),
                "message": message,
                "hint": run.error_hint.clone().unwrap_or_else(|| "Abra o log completo e tente novamente.".into()),
                "source": "run",
                "run_id": run.id,
            }))
        })
        .or_else(|| {
            failed_job.and_then(|job| {
                job.error_message.as_ref().map(|message| {
                    serde_json::json!({
                        "code": "queue_failed",
                        "message": message,
                        "hint": "Use Tentar de novo no controlador da fila ou reexecute a tarefa.",
                        "source": "queue",
                        "job_id": job.id,
                    })
                })
            })
        });

    let latest_log_tail = runs
        .iter()
        .find_map(|run| run.log_path.as_deref())
        .and_then(|path| read_tail(std::path::Path::new(path), 64 * 1024));

    Ok(Json(serde_json::json!({
        "task": task,
        "jobs": jobs,
        "runs": runs,
        "engine": engine,
        "error": error,
        "latest_log_tail": latest_log_tail,
        "generated_at": chrono::Utc::now().to_rfc3339(),
    })))
}

fn read_tail(path: &std::path::Path, max_bytes: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    let start = length.saturating_sub(max_bytes);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).ok()?;
    let text = String::from_utf8_lossy(&buffer).to_string();
    if start > 0
        && let Some(index) = text.find('\n')
    {
        return Some(text[index + 1..].to_string());
    }
    Some(text)
}

#[derive(Debug, Deserialize)]
pub struct TransitionBody {
    #[serde(alias = "column_stage")]
    pub status: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HoldBody {
    pub held: bool,
}

#[derive(Debug, Deserialize, Default)]
pub struct ForkBody {
    pub title: Option<String>,
    pub description: Option<String>,
    pub assigned_agent: Option<String>,
    pub preferred_model: Option<String>,
    pub effort_level: Option<String>,
    pub care_level: Option<String>,
    pub execution_mode: Option<String>,
    pub priority: Option<i64>,
    pub tags: Option<Vec<String>>,
    pub notify_email: Option<String>,
    pub export_to_gdocs: Option<bool>,
}

async fn apply_fork_overrides(
    state: &AppState,
    task: tasks::Task,
    body: Option<ForkBody>,
) -> AppResult<tasks::Task> {
    let Some(body) = body else {
        return Ok(task);
    };
    let title = body
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let description = body
        .description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let model = body
        .preferred_model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let agent = body
        .assigned_agent
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let updated = tasks::update(
        &state.db,
        &task.id,
        UpdateTaskInput {
            title,
            description,
            assigned_agent: agent.clone(),
            preferred_model: model.clone(),
            effort_level: body.effort_level,
            care_level: body.care_level,
            execution_mode: body.execution_mode,
            priority: body.priority,
            tags: body.tags,
            notify_email: body.notify_email,
            export_to_gdocs: body.export_to_gdocs,
            agent_override: agent.map(|_| true),
            model_override: model.map(|_| true),
            ..Default::default()
        },
    )
    .await?;
    Ok(updated)
}

async fn cancel_active_jobs(state: &AppState, task: &tasks::Task) {
    let jobs = queue::list(&state.db, Some(&task.workspace_id))
        .await
        .unwrap_or_default();
    for job in jobs
        .into_iter()
        .filter(|job| job.task_id.as_deref() == Some(task.id.as_str()))
        .filter(|job| matches!(job.status.as_str(), "RUNNING" | "QUEUED" | "PAUSED"))
    {
        let _ = state.queue.cancel(&job.id).await;
    }
}

pub(crate) async fn autostart_enabled(state: &AppState) -> bool {
    settings::bool_flag(&state.db, "tasks_autostart", true).await
}

/// Enqueue a READY task when auto-start is enabled and the engine is usable.
/// Never fails the surrounding operation: if the engine is offline, the task
/// stays in the queue and the reconciler retries later.
pub(crate) async fn try_autostart(
    state: &Arc<AppState>,
    task: &tasks::Task,
) -> Option<tasks::Task> {
    if task.status != "TODO" || task.held {
        return None;
    }
    if matches!(
        task.phase_status.as_deref(),
        Some("awaiting_approval") | Some("queued") | Some("follow_up_queued") | Some("running")
    ) {
        return None;
    }
    if !autostart_enabled(state).await || state.queue.is_paused() {
        return None;
    }
    if let Err(error) = dispatch_task_internal(state, &task.id, DispatchBody::default(), None).await
    {
        tracing::debug!(task = %task.id, error = %error, "task autostart postponed");
        return None;
    }
    tasks::get(&state.db, &task.id).await.ok()
}

/// Explicit status transition. Moving a card to Em execução dispatches a real
/// job; moving to Fila re-opens and auto-starts it; terminal states cancel any
/// queued/running job before changing the column.
pub async fn transition_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<TransitionBody>,
) -> AppResult<Json<serde_json::Value>> {
    let task = tasks::get(&state.db, &id).await?;
    let target = body.status.trim().to_uppercase();

    match target.as_str() {
        "BACKLOG" | "TODO" => {
            cancel_active_jobs(&state, &task).await;
            tasks::set_held(&state.db, &id, false).await?;
            tasks::set_blocked_reason(&state.db, &id, None).await?;
            let task = tasks::replace_phase_status(&state.db, &id, None, Some("TODO")).await?;
            publish_task(&state, &task);
            let task = try_autostart(&state, &task).await.unwrap_or(task);
            publish_task(&state, &task);
            Ok(Json(serde_json::json!({ "task": task, "job": null })))
        }
        "CANCELLED" => {
            cancel_active_jobs(&state, &task).await;
            let task = tasks::set_cancelled(&state.db, &id).await?;
            tasks::set_held(&state.db, &id, false).await?;
            publish_task(&state, &task);
            Ok(Json(serde_json::json!({ "task": task, "job": null })))
        }
        "BLOCKED" => {
            cancel_active_jobs(&state, &task).await;
            tasks::set_held(&state.db, &id, false).await?;
            tasks::set_blocked_reason(
                &state.db,
                &id,
                Some(body.reason.as_deref().unwrap_or("Bloqueada manualmente")),
            )
            .await?;
            let task =
                tasks::replace_phase_status(&state.db, &id, Some("blocked"), Some("BLOCKED"))
                    .await?;
            publish_task(&state, &task);
            Ok(Json(serde_json::json!({ "task": task, "job": null })))
        }
        "DONE" => {
            cancel_active_jobs(&state, &task).await;
            tasks::set_held(&state.db, &id, false).await?;
            tasks::set_blocked_reason(&state.db, &id, None).await?;
            let task =
                tasks::replace_phase_status(&state.db, &id, Some("done"), Some("DONE")).await?;
            publish_task(&state, &task);
            Ok(Json(serde_json::json!({ "task": task, "job": null })))
        }
        "IN_REVIEW" => {
            cancel_active_jobs(&state, &task).await;
            tasks::set_held(&state.db, &id, false).await?;
            let task = tasks::replace_phase_status(
                &state.db,
                &id,
                Some("awaiting_input"),
                Some("IN_REVIEW"),
            )
            .await?;
            publish_task(&state, &task);
            Ok(Json(serde_json::json!({ "task": task, "job": null })))
        }
        "IN_PROGRESS" => {
            if task.active_job_status.is_some() {
                return Ok(Json(serde_json::json!({ "task": task, "job": null })));
            }
            crate::api::engine::require_engine_strict(&state).await?;
            dispatch_task_internal(&state, &id, DispatchBody::default(), None).await?;
            let task = tasks::get(&state.db, &id).await?;
            publish_task(&state, &task);
            Ok(Json(serde_json::json!({ "task": task, "job": null })))
        }
        other => Err(AppError::BadRequest(format!(
            "invalid task status '{other}'"
        ))),
    }
}

pub async fn hold_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<HoldBody>,
) -> AppResult<Json<tasks::Task>> {
    let task = tasks::get(&state.db, &id).await?;
    if body.held {
        cancel_active_jobs(&state, &task).await;
        tasks::set_blocked_reason(&state.db, &id, None).await?;
        tasks::replace_phase_status(&state.db, &id, None, Some("TODO")).await?;
        tasks::set_held(&state.db, &id, true).await?;
        let task = tasks::get(&state.db, &id).await?;
        publish_task(&state, &task);
        Ok(Json(task))
    } else {
        tasks::set_held(&state.db, &id, false).await?;
        let task = tasks::replace_phase_status(&state.db, &id, None, Some("TODO")).await?;
        publish_task(&state, &task);
        let task = try_autostart(&state, &task).await.unwrap_or(task);
        publish_task(&state, &task);
        Ok(Json(task))
    }
}

fn publish_task(state: &AppState, task: &tasks::Task) {
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: task.status.clone(),
        phase_status: task.phase_status.clone(),
    });
}
