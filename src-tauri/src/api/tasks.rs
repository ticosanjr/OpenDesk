use crate::{
    app_state::AppState,
    db::{
        self, budget,
        queue::{self, EnqueueInput},
        router as router_db, runs,
        task_messages::{self, CreateTaskMessageInput},
        tasks::{self, CreateTaskInput, UpdateTaskInput},
        workspaces,
    },
    dsh_router::{self, PackContext, RouterInput},
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

#[derive(Debug, Deserialize, Default)]
pub struct TaskQuery {
    pub workspace_id: Option<String>,
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
    Ok(Json(tasks::list(&state.db, &workspace_id).await?))
}

fn concurrency_group(agent: &str) -> String {
    let agent = agent.to_lowercase();
    if agent.contains("local") || agent.contains("llm") {
        "local_llm_heavy".into()
    } else if agent.contains("explore") || agent.contains("web") {
        "web_scraper".into()
    } else {
        "light_api".into()
    }
}

fn validate_model_ref(value: Option<&str>) -> AppResult<()> {
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
    tasks::delete(&state.db, &id).await?;
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
pub(crate) async fn dispatch_task_internal(
    state: &Arc<AppState>,
    id: &str,
    body: DispatchBody,
    approved_plan: Option<&str>,
) -> AppResult<serde_json::Value> {
    dispatch_task_with_session(state, id, body, approved_plan, None).await
}

pub(crate) async fn dispatch_task_with_session(
    state: &Arc<AppState>,
    id: &str,
    body: DispatchBody,
    approved_plan: Option<&str>,
    session_id: Option<&str>,
) -> AppResult<serde_json::Value> {
    validate_model_ref(body.manual_model.as_deref())?;
    crate::api::engine::require_engine_strict(state).await?;
    let task = tasks::get(&state.db, id).await?;
    let workspace = workspaces::get(&state.db, &task.workspace_id).await?;
    let decision = plan_decision(state, &task, &workspace, &body, approved_plan).await?;
    let job = enqueue_decision_with_session(
        state,
        &task,
        &workspace,
        &decision,
        approved_plan,
        session_id,
    )
    .await?;
    Ok(serde_json::json!({
        "decision": decision,
        "job": job,
    }))
}

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

fn model_key(model: &crate::opencode_client::rest_client::ModelDto) -> String {
    let provider = model.provider_id.clone().unwrap_or_default();
    let id = model.model_id.clone().unwrap_or_else(|| model.id.clone());
    format!("{provider}/{id}")
}

fn base_model(model: &str) -> String {
    let (provider, id) = db::model_health::split_model(model);
    format!("{provider}/{id}")
}

async fn plan_decision(
    state: &AppState,
    task: &tasks::Task,
    workspace: &workspaces::Workspace,
    body: &DispatchBody,
    approved_plan: Option<&str>,
) -> AppResult<dsh_router::RouterDecision> {
    let rules = router_db::list(&state.db, &task.workspace_id).await?;
    let raw_models = state
        .daemon
        .models(Some(&workspace.root_path))
        .await
        .unwrap_or_default();
    let cooling = db::model_health::cooling(&state.db, 24)
        .await
        .unwrap_or_default();
    let filtered = raw_models
        .iter()
        .filter(|model| !cooling.iter().any(|blocked| blocked == &model_key(model)))
        .cloned()
        .collect::<Vec<_>>();
    // If every model is cooling down, keep the full list so the user can still
    // see the real provider error instead of an empty router.
    let available_models = if filtered.is_empty() {
        raw_models
    } else {
        filtered
    };
    let task_model = if task.model_override {
        task.preferred_model
            .as_deref()
            .filter(|model| !cooling.iter().any(|blocked| blocked == &base_model(model)))
    } else {
        None
    };
    let budget_status = budget::status(&state.db).await.ok();
    let policy = body
        .policy
        .clone()
        .unwrap_or_else(|| workspace.default_router_policy.clone());
    let execution_mode = body
        .execution_mode
        .clone()
        .unwrap_or_else(|| task.execution_mode.clone());
    let tags = task.tags();
    let input = RouterInput {
        workspace_id: &task.workspace_id,
        workspace_root: std::path::Path::new(&workspace.root_path),
        title: &task.title,
        prompt: &task.description,
        execution_mode: &execution_mode,
        priority: task.priority,
        tags: &tags,
        manual_agent: body.manual_agent.as_deref().or(if task.agent_override {
            task.assigned_agent.as_deref()
        } else {
            None
        }),
        manual_model: body.manual_model.as_deref().or(task_model),
        manual_effort: body.manual_effort.as_deref(),
        policy: &policy,
        rules: &rules,
        available_models: &available_models,
        approved_plan,
        output_directory: task.output_dir_path.as_deref().map(std::path::Path::new),
        budget_daily_pct: budget_status
            .as_ref()
            .map(|status| status.daily_pct)
            .unwrap_or(0.0),
    };
    Ok(dsh_router::classify(&input))
}

async fn enqueue_decision(
    state: &AppState,
    task: &tasks::Task,
    workspace: &workspaces::Workspace,
    decision: &dsh_router::RouterDecision,
    approved_plan: Option<&str>,
) -> AppResult<crate::db::queue::QueueJob> {
    enqueue_decision_with_session(state, task, workspace, decision, approved_plan, None).await
}

pub(crate) async fn enqueue_decision_with_session(
    state: &AppState,
    task: &tasks::Task,
    workspace: &workspaces::Workspace,
    decision: &dsh_router::RouterDecision,
    approved_plan: Option<&str>,
    session_id: Option<&str>,
) -> AppResult<crate::db::queue::QueueJob> {
    std::fs::create_dir_all(&decision.output_directory).map_err(|error| {
        AppError::Internal(format!("could not create output directory: {error}"))
    })?;
    tasks::set_output_dir(&state.db, &task.id, &decision.output_directory).await?;

    let tags = task.tags();
    let prompt = dsh_router::pack(&PackContext {
        workspace_root: std::path::Path::new(&workspace.root_path),
        output_directory: &decision.output_directory,
        task_title: &task.title,
        task_description: &task.description,
        tags: &tags,
        priority: task.priority,
        phase: &decision.phase,
        plan: approved_plan.or(task.plan_markdown.as_deref()),
        agent: &decision.selected_agent,
    });

    tasks::move_to(&state.db, &task.id, "TODO", None).await?;
    tasks::set_phase(&state.db, &task.id, Some(&decision.phase)).await?;

    let job = state
        .queue
        .enqueue(EnqueueInput {
            workspace_id: task.workspace_id.clone(),
            task_id: Some(task.id.clone()),
            schedule_id: None,
            session_id: session_id.map(str::to_string),
            concurrency_group: Some(concurrency_group(&decision.selected_agent)),
            prompt_payload: prompt,
            resolved_mode: decision.phase.clone(),
            resolved_agent: decision.selected_agent.clone(),
            resolved_model: decision.selected_model.clone(),
            resolved_effort: decision.effort.clone(),
            job_kind: "task".into(),
            scheduled_for: None,
            output_dir: Some(decision.output_directory.clone()),
            priority: task.priority,
            max_retries: 1,
        })
        .await?;

    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: "TODO".into(),
        phase_status: Some("queued".into()),
    });

    Ok(job)
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
) -> AppResult<Json<tasks::Task>> {
    let task = tasks::fork(&state.db, &id).await?;
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
) -> AppResult<Json<tasks::Task>> {
    let run = runs::get(&state.db, &run_id).await?;
    let task_id = run
        .task_id
        .clone()
        .ok_or_else(|| AppError::BadRequest("run sem tarefa associada".into()))?;
    let source = tasks::get(&state.db, &task_id).await?;
    let workspace = workspaces::get(&state.db, &source.workspace_id).await?;
    let forked = tasks::fork_with_model(&state.db, &source.id, None).await?;
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
    let mut existing = std::fs::read_to_string(&path).unwrap_or_default();
    if !existing.ends_with('\n') && !existing.is_empty() {
        existing.push('\n');
    }
    existing.push_str(&format!(
        "- [{}] {}\n",
        chrono::Utc::now().format("%Y-%m-%d"),
        content.replace('\n', " ")
    ));
    std::fs::write(&path, existing)?;
    Ok(Json(serde_json::json!({ "ok": true, "path": path })))
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
    });
    let prompt = format!(
        "{packed}\n\n## Conversa com o usuário\n\n{conversation}\n\nContinue a execução considerando a mensagem acima. Se precisar de uma decisão, pare e faça a pergunta."
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
}

/// Explicit status transition (Paperclip-style). Moving a card never starts a
/// run by itself; dispatch is a separate action. Terminal states cancel any
/// queued/running job for the task.
pub async fn transition_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<TransitionBody>,
) -> AppResult<Json<serde_json::Value>> {
    let task = tasks::get(&state.db, &id).await?;
    let target = body.status.trim().to_uppercase();

    match target.as_str() {
        "BACKLOG" | "CANCELLED" | "BLOCKED" | "TODO" => {
            let jobs = queue::list(&state.db, Some(&task.workspace_id)).await?;
            for job in jobs
                .into_iter()
                .filter(|job| job.task_id.as_deref() == Some(task.id.as_str()))
                .filter(|job| matches!(job.status.as_str(), "RUNNING" | "QUEUED" | "PAUSED"))
            {
                let _ = state.queue.cancel(&job.id).await;
            }
            let task = if target == "CANCELLED" {
                tasks::set_cancelled(&state.db, &id).await?
            } else {
                let reason = if target == "BLOCKED" {
                    Some("Bloqueada manualmente")
                } else {
                    None
                };
                tasks::set_blocked_reason(&state.db, &id, reason).await?;
                tasks::replace_phase_status(
                    &state.db,
                    &id,
                    if target == "BLOCKED" {
                        Some("blocked")
                    } else {
                        None
                    },
                    Some(&target),
                )
                .await?
            };
            publish_task(&state, &task);
            Ok(Json(serde_json::json!({ "task": task, "job": null })))
        }
        "IN_REVIEW" | "DONE" | "IN_PROGRESS" => {
            let task = tasks::replace_phase_status(
                &state.db,
                &id,
                if target == "TODO" {
                    Some("waiting")
                } else {
                    None
                },
                Some(&target),
            )
            .await?;
            publish_task(&state, &task);
            Ok(Json(serde_json::json!({ "task": task, "job": null })))
        }
        other => Err(AppError::BadRequest(format!(
            "invalid task status '{other}'"
        ))),
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
