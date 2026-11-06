use crate::{
    app_state::AppState,
    db::{
        self, budget,
        queue::EnqueueInput,
        router as router_db,
        tasks::{self, CreateTaskInput, UpdateTaskInput},
        workspaces,
    },
    dsh_router::{self, PackContext, RouterInput},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use std::sync::Arc;

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
    pub column_stage: String,
    pub position_index: Option<i64>,
}

pub async fn list_tasks(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TaskQuery>,
) -> AppResult<Json<Vec<tasks::Task>>> {
    let workspace_id = resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(tasks::list(&state.db, &workspace_id).await?))
}

pub async fn create_task(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TaskQuery>,
    Json(input): Json<CreateTaskInput>,
) -> AppResult<Json<tasks::Task>> {
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
    let task = tasks::update(&state.db, &id, input).await?;
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: task.column_stage.clone(),
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
    let task = tasks::move_to(&state.db, &id, &body.column_stage, body.position_index).await?;
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: task.column_stage.clone(),
        phase_status: task.phase_status.clone(),
    });
    Ok(Json(task))
}

pub async fn dispatch_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<DispatchBody>,
) -> AppResult<Json<serde_json::Value>> {
    let task = tasks::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &task.workspace_id).await?;
    let decision = plan_decision(&state, &task, &workspace, &body, None).await?;
    let job = enqueue_decision(&state, &task, &workspace, &decision, None).await?;
    Ok(Json(serde_json::json!({
        "decision": decision,
        "job": job,
    })))
}

pub async fn approve_plan(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<DispatchBody>,
) -> AppResult<Json<serde_json::Value>> {
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

async fn plan_decision(
    state: &AppState,
    task: &tasks::Task,
    workspace: &workspaces::Workspace,
    body: &DispatchBody,
    approved_plan: Option<&str>,
) -> AppResult<dsh_router::RouterDecision> {
    let rules = router_db::list(&state.db, &task.workspace_id).await?;
    let available_models = state
        .daemon
        .models(Some(&workspace.root_path))
        .await
        .unwrap_or_default();
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
        manual_agent: body
            .manual_agent
            .as_deref()
            .or(task.assigned_agent.as_deref()),
        manual_model: body
            .manual_model
            .as_deref()
            .or(task.preferred_model.as_deref()),
        manual_effort: body
            .manual_effort
            .as_deref()
            .or(Some(task.effort_level.as_str())),
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

    let updated = tasks::update(
        &state.db,
        &task.id,
        UpdateTaskInput {
            assigned_agent: Some(decision.selected_agent.clone()),
            preferred_model: Some(decision.selected_model.clone()),
            effort_level: Some(decision.effort.clone()),
            ..Default::default()
        },
    )
    .await?;
    tasks::move_to(
        &state.db,
        &task.id,
        if decision.phase == "plan" {
            "PLAN"
        } else {
            "BUILD"
        },
        None,
    )
    .await?;

    let job = state
        .queue
        .enqueue(EnqueueInput {
            workspace_id: task.workspace_id.clone(),
            task_id: Some(task.id.clone()),
            schedule_id: None,
            prompt_payload: prompt,
            resolved_mode: decision.phase.clone(),
            resolved_agent: decision.selected_agent.clone(),
            resolved_model: decision.selected_model.clone(),
            resolved_effort: decision.effort.clone(),
            job_kind: "task".into(),
            scheduled_for: None,
            output_dir: Some(decision.output_directory.clone()),
            priority: task.priority,
            max_retries: 2,
        })
        .await?;

    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: if decision.phase == "plan" {
            "PLAN"
        } else {
            "BUILD"
        }
        .into(),
        phase_status: Some("queued".into()),
    });

    let _ = updated;
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
