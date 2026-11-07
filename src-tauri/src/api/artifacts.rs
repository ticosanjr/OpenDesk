use crate::{
    app_state::AppState,
    db::{
        artifacts::{self, ArtifactProvenance},
        workspaces,
    },
    error::{AppError, AppResult},
};
use axum::{
    Json,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, header},
    response::Response,
};
use serde::Deserialize;
use std::path::Path as FsPath;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct ArtifactQuery {
    pub workspace_id: Option<String>,
    pub agent: Option<String>,
    pub task_id: Option<String>,
    pub run_id: Option<String>,
    pub action: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct ArtifactContent {
    pub path: String,
    pub content: String,
    pub truncated: bool,
    pub binary: bool,
    pub mime: String,
    pub size: u64,
}

pub async fn list_artifacts(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ArtifactQuery>,
) -> AppResult<Json<Vec<ArtifactProvenance>>> {
    let workspace_id = match query.workspace_id {
        Some(id) => id,
        None => crate::api::tasks::resolve_workspace_id(&state, None).await?,
    };
    let list = if let Some(run_id) = query.run_id.as_deref() {
        artifacts::list_by_run(&state.db, run_id, query.limit.unwrap_or(200)).await?
    } else {
        artifacts::list(
            &state.db,
            &workspace_id,
            query.agent.as_deref(),
            query.task_id.as_deref(),
            query.action.as_deref(),
            query.limit.unwrap_or(200),
        )
        .await?
    };
    Ok(Json(list))
}

pub async fn pinned_artifacts(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ArtifactQuery>,
) -> AppResult<Json<Vec<ArtifactProvenance>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        artifacts::list_pinned(&state.db, &workspace_id, 50).await?,
    ))
}

pub async fn get_artifact(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<ArtifactProvenance>> {
    Ok(Json(artifacts::get(&state.db, &id).await?))
}

pub async fn artifact_content(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<ArtifactContent>> {
    let artifact = artifacts::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &artifact.workspace_id).await?;
    let root = std::fs::canonicalize(&workspace.root_path)
        .map_err(|error| AppError::Internal(format!("workspace path: {error}")))?;
    let file = std::fs::canonicalize(&artifact.absolute_path)
        .map_err(|error| AppError::NotFound(format!("artifact file: {error}")))?;

    if !file.starts_with(&root) {
        return Err(AppError::BadRequest(
            "artifact path is outside the workspace".into(),
        ));
    }

    let bytes = std::fs::read(&file)
        .map_err(|error| AppError::Internal(format!("could not read artifact: {error}")))?;
    let truncated = bytes.len() > 2 * 1024 * 1024;
    let slice = if truncated {
        &bytes[..2 * 1024 * 1024]
    } else {
        &bytes
    };
    let binary =
        slice.iter().take(8_000).any(|byte| *byte == 0) || std::str::from_utf8(slice).is_err();
    let content = if binary {
        String::new()
    } else {
        String::from_utf8_lossy(slice).to_string()
    };

    Ok(Json(ArtifactContent {
        path: FsPath::new(&artifact.absolute_path)
            .to_string_lossy()
            .to_string(),
        content,
        truncated,
        binary,
        mime: crate::api::files::mime_for(&file).to_string(),
        size: bytes.len() as u64,
    }))
}

#[derive(Debug, Deserialize)]
pub struct PinBody {
    pub pinned: bool,
}

pub async fn pin_artifact(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<PinBody>,
) -> AppResult<Json<ArtifactProvenance>> {
    Ok(Json(
        artifacts::set_pinned(&state.db, &id, body.pinned).await?,
    ))
}

#[derive(Debug, serde::Serialize)]
pub struct RollbackResult {
    pub ok: bool,
    pub path: String,
    pub action: String,
}

pub async fn rollback_artifact(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<RollbackResult>> {
    let artifact = artifacts::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &artifact.workspace_id).await?;
    let root = std::fs::canonicalize(&workspace.root_path)
        .map_err(|error| AppError::Internal(format!("workspace path: {error}")))?;
    let file = std::fs::canonicalize(&artifact.absolute_path)
        .map_err(|error| AppError::NotFound(format!("artifact file: {error}")))?;
    if !file.starts_with(&root) {
        return Err(AppError::BadRequest(
            "artifact path is outside the workspace".into(),
        ));
    }

    match artifact.action_type.as_str() {
        "CREATED" => {
            std::fs::remove_file(&file)?;
        }
        "MODIFIED" => {
            let patch = artifact.diff_patch.as_deref().unwrap_or_default();
            let (old, new) = split_patch(patch);
            if new.is_empty() {
                return Err(AppError::BadRequest(
                    "diff de artefato não contém o texto aplicado".into(),
                ));
            }
            let current = std::fs::read_to_string(&file)?;
            let reverted = current.replacen(&new, &old, 1);
            if reverted == current {
                return Err(AppError::Conflict(
                    "o arquivo mudou desde o artefato; rollback não é seguro".into(),
                ));
            }
            std::fs::write(&file, reverted)?;
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "ação '{other}' não tem rollback automático"
            )));
        }
    }

    Ok(Json(RollbackResult {
        ok: true,
        path: artifact.absolute_path.clone(),
        action: artifact.action_type,
    }))
}

/// Reconstruct the old/new text recorded by the edit/write tool so the
/// provenance entry can be reverted without an external patch binary.
fn split_patch(patch: &str) -> (String, String) {
    let mut old = Vec::new();
    let mut new = Vec::new();
    for line in patch.lines() {
        if line.starts_with("--- ") || line.starts_with("+++ ") || line.starts_with("@@") {
            continue;
        }
        if let Some(rest) = line.strip_prefix('+') {
            new.push(rest);
        } else if let Some(rest) = line.strip_prefix('-') {
            old.push(rest);
        }
    }
    (old.join("\n"), new.join("\n"))
}

pub async fn artifact_raw(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> AppResult<Response> {
    let artifact = artifacts::get(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &artifact.workspace_id).await?;
    let root = std::fs::canonicalize(&workspace.root_path)
        .map_err(|error| AppError::Internal(format!("workspace path: {error}")))?;
    let file = std::fs::canonicalize(&artifact.absolute_path)
        .map_err(|error| AppError::NotFound(format!("artifact file: {error}")))?;
    if !file.starts_with(&root) {
        return Err(AppError::BadRequest(
            "artifact path is outside the workspace".into(),
        ));
    }
    let range_header = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());
    let (bytes, total, range) = crate::api::files::read_raw(&file, range_header)?;
    let filename = file
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "artifact".into());
    let mut builder = Response::builder()
        .header(header::CONTENT_TYPE, crate::api::files::mime_for(&file))
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{filename}\""),
        )
        .header(header::CONTENT_LENGTH, bytes.len())
        .header(header::ACCEPT_RANGES, "bytes");
    if let Some((start, end)) = range {
        builder = builder
            .status(axum::http::StatusCode::PARTIAL_CONTENT)
            .header(
                header::CONTENT_RANGE,
                format!("bytes {start}-{end}/{total}"),
            );
    }
    builder
        .body(Body::from(bytes))
        .map_err(|error| AppError::Internal(format!("could not build response: {error}")))
}

#[cfg(test)]
mod tests {
    use super::split_patch;

    #[test]
    fn splits_recorded_edit_patch() {
        let patch = "--- a/src/main.rs\n+++ b/src/main.rs\n-old line\n+new line\n";
        let (old, new) = split_patch(patch);
        assert_eq!(old, "old line");
        assert_eq!(new, "new line");
    }

    #[test]
    fn splits_multiline_edit_patch() {
        let patch = "--- a/a.txt\n+++ b/a.txt\n-um\n-dois\n+tres\n+quatro\n";
        let (old, new) = split_patch(patch);
        assert_eq!(old, "um\ndois");
        assert_eq!(new, "tres\nquatro");
    }
}
