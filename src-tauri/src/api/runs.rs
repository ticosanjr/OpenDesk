use crate::{
    app_state::AppState,
    db::{
        artifacts, queue,
        runs::{self, TaskRun},
        tasks, workspaces,
    },
    error::{AppError, AppResult},
    run_timeline::{RunTimeline, TimelineArtifact, timeline_from_events},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::header,
    response::IntoResponse,
};
use serde::Deserialize;
use std::{io::BufRead, sync::Arc};

#[derive(Debug, Deserialize, Default)]
pub struct RunQuery {
    pub workspace_id: Option<String>,
    pub task_id: Option<String>,
    pub limit: Option<i64>,
}

pub async fn list_runs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RunQuery>,
) -> AppResult<Json<Vec<TaskRun>>> {
    let workspace_id = match query.workspace_id {
        Some(id) => id,
        None => crate::api::tasks::resolve_workspace_id(&state, None).await?,
    };
    Ok(Json(
        runs::list_with_limit(
            &state.db,
            &workspace_id,
            query.task_id.as_deref(),
            query.limit.unwrap_or(100),
        )
        .await?,
    ))
}

pub async fn get_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<TaskRun>> {
    Ok(Json(runs::get(&state.db, &id).await?))
}

pub async fn run_log(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let run = runs::get(&state.db, &id).await?;
    let path = run
        .log_path
        .ok_or_else(|| AppError::NotFound("this run has no log file".into()))?;
    let content = std::fs::read(&path)
        .map_err(|error| AppError::Internal(format!("could not read run log: {error}")))?;
    let start = content.len().saturating_sub(256 * 1024);
    let body = String::from_utf8_lossy(&content[start..]).to_string();
    Ok(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body))
}

pub async fn run_timeline(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<RunTimeline>> {
    let run = runs::get(&state.db, &id).await?;
    let path = run.log_path.clone();
    let events = path
        .as_deref()
        .and_then(|path| std::fs::File::open(path).ok())
        .map(|file| {
            std::io::BufReader::new(file)
                .lines()
                .map_while(Result::ok)
                .filter_map(|line| serde_json::from_str::<serde_json::Value>(&line).ok())
                .collect::<Vec<_>>()
        });
    let log_missing = events.is_none();
    let mut timeline =
        timeline_from_events(Some(run.clone()), events.unwrap_or_default(), log_missing);

    let workspace = workspaces::get(&state.db, &run.workspace_id).await.ok();
    if let Some(workspace) = &workspace {
        timeline.workspace_root = Some(workspace.root_path.clone());
    }
    let mut output_directory = match run.task_id.as_deref() {
        Some(task_id) => tasks::get(&state.db, task_id)
            .await
            .ok()
            .and_then(|task| task.output_dir_path),
        None => None,
    };
    if output_directory.is_none()
        && let Some(job_id) = run.queue_job_id.as_deref()
    {
        output_directory = queue::get(&state.db, job_id)
            .await
            .ok()
            .and_then(|job| job.output_dir);
    }
    if let Some(output) = &output_directory {
        timeline.output_directory = Some(output.clone());
        if let Some(workspace) = &workspace {
            timeline.output_directory_relative = output
                .strip_prefix(&workspace.root_path)
                .map(str::to_string);
        }
    }
    timeline.artifacts = artifacts::list_by_run(&state.db, &id, 200)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|artifact| TimelineArtifact {
            id: artifact.id,
            file_path: artifact.file_path,
            absolute_path: artifact.absolute_path,
            action_type: artifact.action_type,
            created_at: artifact.created_at,
        })
        .collect();
    Ok(Json(timeline))
}
