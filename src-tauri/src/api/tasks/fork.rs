use crate::{
    app_state::AppState,
    db::{
        self,
        tasks::{self, UpdateTaskInput},
        workspaces,
    },
    dsh_router,
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use std::sync::Arc;

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

/// Fork a task preserving the deliverables produced up to a specific run.
/// Files are copied into a fresh output folder so the original stays intact.
pub async fn fork_from_run(
    State(state): State<Arc<AppState>>,
    Path(run_id): Path<String>,
    body: Option<Json<ForkBody>>,
) -> AppResult<Json<tasks::Task>> {
    let run = db::runs::get(&state.db, &run_id).await?;
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
