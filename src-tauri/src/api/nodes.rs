use crate::{
    app_state::AppState,
    db::{queue, runs, tasks, workspaces},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
    nodes::{self, HeartbeatInput, NodeView, UpdateNodeInput},
};
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::IntoResponse,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};

async fn node_from_headers(state: &AppState, headers: &HeaderMap) -> AppResult<nodes::Worker> {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::BadRequest("token de nó ausente".into()))?;
    nodes::authenticate(&state.db, token)
        .await?
        .ok_or_else(|| AppError::BadRequest("token de nó inválido".into()))
}

pub async fn list(State(state): State<Arc<AppState>>) -> AppResult<Json<Vec<NodeView>>> {
    Ok(Json(nodes::list(&state.db).await?))
}

#[derive(Debug, Deserialize)]
pub struct CreateNodeBody {
    pub name: String,
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateNodeBody>,
) -> AppResult<Json<Value>> {
    let (token, node) =
        nodes::create(&state.db, &body.name, &body.endpoint, body.capabilities).await?;
    Ok(Json(json!({ "token": token, "node": node })))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<UpdateNodeInput>,
) -> AppResult<Json<NodeView>> {
    Ok(Json(nodes::update(&state.db, &id, input).await?))
}

pub async fn rotate(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let (token, node) = nodes::rotate_token(&state.db, &id).await?;
    Ok(Json(json!({ "token": token, "node": node })))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    if !nodes::delete(&state.db, &id).await? {
        return Err(AppError::NotFound(format!("node {id}")));
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn heartbeat(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(input): Json<HeartbeatInput>,
) -> AppResult<Json<NodeView>> {
    let token = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or("");
    Ok(Json(
        nodes::heartbeat(&state.db, &state.write_buffer, token, input).await?,
    ))
}

pub async fn next_job(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    let worker = node_from_headers(&state, &headers).await?;
    let _ = queue::requeue_expired(&state.db).await?;
    let Some(job) = queue::claim_for_node(&state.db, &worker.id, 120).await? else {
        return Ok(Json(json!({ "job": null })));
    };
    let workspace = match workspaces::get(&state.db, &job.workspace_id).await {
        Ok(workspace) => workspace,
        Err(error) => {
            let _ = queue::finish(&state.db, &job.id, "FAILED", Some(&error.to_string())).await;
            return Err(error);
        }
    };
    let (provider_id, model_id) = job
        .resolved_model
        .split_once('/')
        .map(|(provider, model)| (Some(provider.to_string()), model.to_string()))
        .unwrap_or((None, job.resolved_model.clone()));
    let run_id = uuid::Uuid::now_v7().to_string();
    if let Err(error) = runs::create(
        &state.db,
        runs::CreateRunInput {
            id: run_id.clone(),
            queue_job_id: Some(job.id.clone()),
            workspace_id: job.workspace_id.clone(),
            task_id: job.task_id.clone(),
            schedule_id: job.schedule_id.clone(),
            parent_run_id: None,
            phase: Some(job.resolved_mode.clone()),
            agent_name: job.resolved_agent.clone(),
            execution_mode: job.resolved_mode.clone(),
            provider_id,
            model_id,
            effort_level: job.resolved_effort.clone(),
            pricing_snapshot_json: None,
            log_path: None,
        },
    )
    .await
    {
        let _ = queue::finish(&state.db, &job.id, "FAILED", Some(&error.to_string())).await;
        return Err(error);
    }
    let sandbox = crate::sandbox::get_for(&state.db, Some(&job.workspace_id))
        .await
        .unwrap_or_default();
    Ok(Json(json!({
        "job": job,
        "run_id": run_id,
        "workspace_root": workspace.root_path,
        "sandbox": sandbox,
    })))
}

#[derive(Debug, Default, Deserialize)]
pub struct NodeSyncBody {
    #[serde(default)]
    pub base: Vec<crate::workspace_sync::FileEntry>,
}

async fn assigned_job(
    state: &AppState,
    headers: &HeaderMap,
    id: &str,
) -> AppResult<(nodes::Worker, queue::QueueJob, workspaces::Workspace)> {
    let worker = node_from_headers(state, headers).await?;
    let job = queue::get(&state.db, id).await?;
    if job.assigned_node_id.as_deref() != Some(worker.id.as_str()) {
        return Err(AppError::Conflict("job pertence a outro nó".into()));
    }
    let workspace = workspaces::get(&state.db, &job.workspace_id).await?;
    Ok((worker, job, workspace))
}

/// Builds a delta bundle for a node work directory. The node sends its local
/// manifest; the controller returns only added/changed files.
pub async fn sync_bundle(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<NodeSyncBody>,
) -> AppResult<Json<Value>> {
    let (_worker, job, workspace) = assigned_job(&state, &headers, &id).await?;
    let root = PathBuf::from(workspace.root_path);
    let destination = state
        .paths
        .data_dir
        .join("sync")
        .join(format!("{}.tar.zst", uuid::Uuid::now_v7()));
    let base = body.base;
    let destination_for_task = destination.clone();
    let (current, delta, report) = tokio::task::spawn_blocking(move || {
        let current = crate::workspace_sync::manifest(&root)?;
        let delta = crate::workspace_sync::delta(&base, &current);
        let files = delta
            .added
            .iter()
            .chain(delta.changed.iter())
            .cloned()
            .collect::<Vec<_>>();
        let report = crate::workspace_sync::bundle(&root, &files, &destination_for_task)?;
        AppResult::Ok((current, delta, report))
    })
    .await
    .map_err(|error| AppError::Internal(format!("sync task: {error}")))??;
    let bundle_id = destination
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| AppError::Internal("bundle sem nome".into()))?;
    Ok(Json(json!({
        "job_id": job.id,
        "bundle_id": bundle_id,
        "delta": delta,
        "manifest": current,
        "files": report.files,
        "bytes": report.bytes,
    })))
}

pub async fn download_sync(
    State(state): State<Arc<AppState>>,
    Path((id, bundle_id)): Path<(String, String)>,
    headers: HeaderMap,
) -> AppResult<impl IntoResponse> {
    let (_worker, _job, _workspace) = assigned_job(&state, &headers, &id).await?;
    if bundle_id.contains('/') || bundle_id.contains('\\') {
        return Err(AppError::BadRequest("bundle_id inválido".into()));
    }
    let path = state.paths.data_dir.join("sync").join(bundle_id);
    let path = tokio::task::spawn_blocking(move || {
        crate::workspace_sync::validate_bundle_path(&path, &state.paths.data_dir)
    })
    .await
    .map_err(|error| AppError::Internal(format!("sync task: {error}")))??;
    let bytes = tokio::fs::read(path).await?;
    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
        Bytes::from(bytes),
    ))
}

pub async fn job_heartbeat(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> AppResult<Json<Value>> {
    let worker = node_from_headers(&state, &headers).await?;
    let job = queue::get(&state.db, &id).await?;
    if job.assigned_node_id.as_deref() != Some(worker.id.as_str()) {
        return Err(AppError::Conflict("job pertence a outro nó".into()));
    }
    state.write_buffer.job_heartbeat(&id).await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Default, Deserialize)]
pub struct UsageBody {
    #[serde(default)]
    pub tokens_input: i64,
    #[serde(default)]
    pub tokens_output: i64,
    #[serde(default)]
    pub tokens_reasoning: i64,
    #[serde(default)]
    pub tokens_cache_read: i64,
    #[serde(default)]
    pub tokens_cache_write: i64,
    #[serde(default)]
    pub estimated_cost_usd: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteFile {
    pub path: String,
    #[serde(default = "default_file_status")]
    pub status: String,
    #[serde(default)]
    pub patch: String,
    /// Optional full content for a node without a shared mount.
    #[serde(default)]
    pub content_base64: Option<String>,
}

fn default_file_status() -> String {
    "modified".into()
}

#[derive(Debug, Deserialize)]
pub struct CompleteBody {
    pub status: String,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub duration_ms: i64,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub usage: UsageBody,
    #[serde(default)]
    pub files: Vec<RemoteFile>,
}

pub async fn complete_job(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(body): Json<CompleteBody>,
) -> AppResult<Json<Value>> {
    let worker = node_from_headers(&state, &headers).await?;
    let job = queue::get(&state.db, &id).await?;
    if job.assigned_node_id.as_deref() != Some(worker.id.as_str()) {
        return Err(AppError::Conflict("job pertence a outro nó".into()));
    }
    let queue_status = match body.status.to_lowercase().as_str() {
        "completed" | "success" | "succeeded" => "COMPLETED",
        "cancelled" | "canceled" => "CANCELLED",
        _ => "FAILED",
    };
    queue::finish(&state.db, &id, queue_status, body.error.as_deref()).await?;

    let run_id = match body.run_id {
        Some(run_id) => Some(run_id),
        None => {
            sqlx::query_scalar::<_, String>(
                "SELECT id FROM task_runs WHERE queue_job_id = ? AND status = 'RUNNING'
             ORDER BY started_at DESC LIMIT 1",
            )
            .bind(&id)
            .fetch_optional(&state.db)
            .await?
        }
    };
    if let Some(run_id) = run_id.as_deref() {
        let usage = runs::RunUsage {
            tokens_input: body.usage.tokens_input,
            tokens_output: body.usage.tokens_output,
            tokens_reasoning: body.usage.tokens_reasoning,
            tokens_cache_read: body.usage.tokens_cache_read,
            tokens_cache_write: body.usage.tokens_cache_write,
            estimated_cost_usd: body.usage.estimated_cost_usd,
        };
        let run_status = match queue_status {
            "COMPLETED" => "SUCCESS",
            "CANCELLED" => "CANCELLED",
            _ => "FAILED",
        };
        let _ = runs::finish(
            &state.db,
            run_id,
            run_status,
            &usage,
            body.duration_ms.max(0),
            body.summary.as_deref().or(body.error.as_deref()),
            None,
        )
        .await;
        state.events.publish(OpenDeskWsEvent::RunFinished {
            run_id: run_id.to_string(),
            task_id: job.task_id.clone(),
            status: run_status.to_string(),
            estimated_cost_usd: usage.estimated_cost_usd,
            tokens_input: usage.tokens_input,
            tokens_output: usage.tokens_output,
        });
    }
    if let Some(run_id) = run_id.as_deref()
        && !body.files.is_empty()
        && let Ok(workspace) = workspaces::get(&state.db, &job.workspace_id).await
    {
        let workspace_root = std::path::Path::new(&workspace.root_path);
        let reason = format!("worker {} · {}", worker.name, job.resolved_mode);
        for file in body.files.iter().take(200) {
            let path = std::path::Path::new(&file.path);
            if path.is_absolute()
                || path
                    .components()
                    .any(|component| matches!(component, std::path::Component::ParentDir))
            {
                continue;
            }
            let action = match file.status.as_str() {
                "added" => "CREATED",
                "deleted" => "DELETED",
                _ => "MODIFIED",
            };
            if file.status == "deleted" {
                let target = workspace_root.join(path);
                if target.is_file() {
                    let _ = std::fs::remove_file(target);
                }
            } else if let Some(content) = file.content_base64.as_deref()
                && let Ok(bytes) =
                    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, content)
            {
                let target = workspace_root.join(path);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                    let canonical_parent = std::fs::canonicalize(parent)?;
                    let canonical_root = std::fs::canonicalize(workspace_root)?;
                    if !canonical_parent.starts_with(&canonical_root) {
                        return Err(AppError::BadRequest(format!(
                            "recusa escrever fora do workspace: {}",
                            file.path
                        )));
                    }
                }
                if target
                    .symlink_metadata()
                    .map(|meta| meta.file_type().is_symlink())
                    .unwrap_or(false)
                {
                    return Err(AppError::BadRequest(format!(
                        "recusa escrever em symlink: {}",
                        file.path
                    )));
                }
                std::fs::write(&target, bytes)?;
            }
            crate::queue_scheduler::run_execution::record_artifact(
                &state.db,
                &state.events,
                &job.workspace_id,
                job.task_id.as_deref(),
                run_id,
                &job.resolved_agent,
                &reason,
                workspace_root,
                &file.path,
                action,
                (!file.patch.is_empty()).then_some(file.patch.as_str()),
                None,
            )
            .await;
        }
    }

    if let Some(task_id) = job.task_id.as_deref() {
        let task_status = match queue_status {
            "COMPLETED" => "DONE",
            "CANCELLED" => "CANCELLED",
            _ => "BLOCKED",
        };
        let _ = tasks::set_last_run(&state.db, task_id, run_id.as_deref(), Some(task_status)).await;
        if let Ok(task) = tasks::get(&state.db, task_id).await {
            state.events.publish(OpenDeskWsEvent::TaskUpdated {
                task_id: task.id.clone(),
                workspace_id: task.workspace_id.clone(),
                stage: task_status.to_string(),
                phase_status: Some(queue_status.to_lowercase()),
            });
        }
    }
    let jobs = queue::list(&state.db, Some(&job.workspace_id)).await?;
    state.events.publish(OpenDeskWsEvent::QueueUpdated {
        workspace_id: job.workspace_id.clone(),
        jobs: jobs
            .into_iter()
            .map(crate::queue_scheduler::queue_manager::job_dto)
            .collect(),
    });
    Ok(Json(json!({
        "ok": true,
        "queue_status": queue_status,
        "run_id": run_id,
    })))
}
