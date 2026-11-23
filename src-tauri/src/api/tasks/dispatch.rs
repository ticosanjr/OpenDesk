use crate::{
    app_state::AppState,
    db::{
        self, runs,
        tasks::{self},
        workspaces,
    },
    dsh_router,
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

use super::task_dispatch::{base_model, enqueue_decision, model_key, plan_decision};
use super::validate_model_ref;

#[derive(Debug, Deserialize, Default)]
pub struct DispatchBody {
    pub policy: Option<String>,
    pub manual_agent: Option<String>,
    pub manual_model: Option<String>,
    pub manual_effort: Option<String>,
    pub execution_mode: Option<String>,
    /// One-off instructions for this execution. Never persisted on the task.
    #[serde(default)]
    pub prompt_append: Option<String>,
}

pub async fn dispatch_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<DispatchBody>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(
        super::dispatch_task_internal(&state, &id, body, None).await?,
    ))
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
            prompt_append: None,
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
                prompt_append: None,
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
