use crate::{
    app_state::AppState,
    db::{
        queue::EnqueueInput,
        runs,
        task_messages::{self, CreateTaskMessageInput},
        tasks, workspaces,
    },
    dsh_router::{self, PackContext},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
};
use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

use super::task_dispatch::{concurrency_group, plan_decision};
use super::{DispatchBody, validate_model_ref};

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
        &state.db,
        &state.paths,
        &task.workspace_id,
        &task.title,
        &task.description,
        &tags,
    )
    .await?;
    let personal_context =
        crate::personal_context::render_body(&state.db, &task.workspace_id).await?;
    let memory_context = crate::db::memory::render_context(
        &state.db,
        &task.workspace_id,
        &decision.selected_agent,
        &format!("{} {}", task.title, task.description),
        8,
    )
    .await
    .unwrap_or_default();
    let session_packet = crate::session_context::render_and_persist(&state.db, &task.id)
        .await
        .unwrap_or_default();
    let templates = crate::dsh_router::prompt_templates::PromptTemplates::load(
        &state.db,
        Some(&task.workspace_id),
    )
    .await?;
    let packed = dsh_router::pack(
        &PackContext {
            workspace_root: std::path::Path::new(&workspace.root_path),
            output_directory: &decision.output_directory,
            task_id: &task.id,
            task_title: &task.title,
            task_description: &task.description,
            tags: &tags,
            priority: task.priority,
            phase: "follow_up",
            plan: task.plan_markdown.as_deref(),
            agent: &decision.selected_agent,
            care_level: &task.care_level,
            needs_browser,
            personal_context: &personal_context,
            structured_memory: &memory_context,
            session_packet: &session_packet,
        },
        &templates,
    );
    let follow_up = crate::dsh_router::prompt_templates::render(
        &templates.follow_up,
        &[("conversation", &conversation)],
    );
    let prompt = format!("{packed}\n\n{follow_up}");

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
            resolved_harness: decision.harness.clone(),
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
