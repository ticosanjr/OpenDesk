use super::prompt_layer;
use crate::{
    app_state::AppState,
    db::{tasks, workspaces},
    dsh_router::{self, PackContext},
    error::AppResult,
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Serialize)]
pub struct PromptPreview {
    pub task_id: String,
    pub workspace_id: String,
    pub phase: String,
    pub agent: String,
    pub model: String,
    pub output_directory: String,
    pub prompt: String,
    pub sections: Vec<crate::dsh_router::context_packer::PromptSection>,
    pub opencode: prompt_layer::OpenCodeLayer,
    /// OpenDesk prompt plus everything read from the OpenCode native files. It
    /// is a reconstruction, not the provider wire payload.
    pub composed: String,
}

pub async fn preview(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<PromptPreview>> {
    let task = tasks::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &task.workspace_id).await?;
    let tags = task.tags();
    let body = crate::api::tasks::DispatchBody {
        policy: None,
        manual_agent: task
            .agent_override
            .then(|| task.assigned_agent.clone())
            .flatten(),
        manual_model: task
            .model_override
            .then(|| task.preferred_model.clone())
            .flatten(),
        manual_effort: None,
        execution_mode: Some(task.execution_mode.clone()),
        prompt_append: None,
    };
    let decision = crate::api::tasks::task_dispatch::plan_decision(
        &state,
        &task,
        &workspace,
        &body,
        task.plan_markdown.as_deref(),
    )
    .await?;
    let personal_context =
        crate::personal_context::render_body(&state.db, &task.workspace_id).await?;
    let templates = crate::dsh_router::prompt_templates::PromptTemplates::load(
        &state.db,
        Some(&task.workspace_id),
    )
    .await?;
    let context = PackContext {
        workspace_root: std::path::Path::new(&workspace.root_path),
        output_directory: &decision.output_directory,
        task_id: &task.id,
        task_title: &task.title,
        task_description: &task.description,
        tags: &tags,
        priority: task.priority,
        phase: &decision.phase,
        plan: task.plan_markdown.as_deref(),
        agent: &decision.selected_agent,
        care_level: &task.care_level,
        needs_browser: crate::browse::needs_browser(&task.title, &task.description, &tags),
        personal_context: &personal_context,
    };
    let sections = dsh_router::context_packer::pack_sections(&context, &templates);
    let prompt = sections
        .iter()
        .map(|section| section.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    let layer = prompt_layer::snapshot(
        std::path::Path::new(&workspace.root_path),
        &decision.selected_agent,
    );
    let composed = prompt_layer::compose(&prompt, &layer);

    Ok(Json(PromptPreview {
        task_id: task.id,
        workspace_id: task.workspace_id,
        phase: decision.phase,
        agent: decision.selected_agent,
        model: decision.selected_model,
        output_directory: decision.output_directory,
        prompt,
        sections,
        opencode: layer,
        composed,
    }))
}
