use crate::{
    app_state::AppState,
    db::{self, budget, queue::EnqueueInput, router as router_db, tasks, workspaces},
    dsh_router::{self, PackContext, RouterInput},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
};
use std::sync::Arc;

use super::{DispatchBody, validate_model_ref};

pub(super) fn concurrency_group(agent: &str) -> String {
    let agent = agent.to_lowercase();
    if agent.contains("local") || agent.contains("llm") {
        "local_llm_heavy".into()
    } else if agent.contains("explore") || agent.contains("web") {
        "web_scraper".into()
    } else {
        "light_api".into()
    }
}

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
    let task = tasks::get(&state.db, id).await?;
    let workspace = workspaces::get(&state.db, &task.workspace_id).await?;
    if let Some(agent) = body.manual_agent.as_deref() {
        crate::governance::check_budget(&state.db, &task.workspace_id, agent).await?;
    }
    crate::api::engine::require_engine_strict(state).await?;
    let decision = plan_decision(state, &task, &workspace, &body, approved_plan).await?;
    crate::governance::check_budget(&state.db, &task.workspace_id, &decision.selected_agent)
        .await?;
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

pub(super) fn model_key(model: &crate::opencode_client::rest_client::ModelDto) -> String {
    let provider = model.provider_id.clone().unwrap_or_default();
    let id = model.model_id.clone().unwrap_or_else(|| model.id.clone());
    format!("{provider}/{id}")
}

pub(super) fn base_model(model: &str) -> String {
    let (provider, id) = db::model_health::split_model(model);
    format!("{provider}/{id}")
}

pub(super) async fn plan_decision(
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
    // A model chosen by the user is never replaced by the router, even when it
    // is cooling down: failing with the provider error is clearer than silently
    // running on a different model.
    let task_model = if task.model_override {
        task.preferred_model.as_deref()
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
    let default_model =
        dsh_router::configured_default_model(std::path::Path::new(&workspace.root_path));
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
        default_model: default_model.as_deref(),
        approved_plan,
        output_directory: task.output_dir_path.as_deref().map(std::path::Path::new),
        budget_daily_pct: budget_status
            .as_ref()
            .map(|status| status.daily_pct)
            .unwrap_or(0.0),
    };
    Ok(dsh_router::classify(&input))
}

pub(super) async fn enqueue_decision(
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
    let needs_browser = crate::browse::ensure_for_task(
        state,
        &task.workspace_id,
        &task.title,
        &task.description,
        &tags,
    )
    .await?;
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
        needs_browser,
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
