use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};
use axum::{
    body::Body,
    extract::{Query, State},
    http::{StatusCode, header},
    response::Response,
};
use std::{path::Path, sync::Arc};

use super::paths::{FileQuery, resolve_path, workspace_root};

pub async fn raw(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileQuery>,
) -> AppResult<Response> {
    let root = workspace_root(&state, query.workspace_id.as_deref()).await?;
    let path = resolve_path(&root, query.path.as_deref())?;
    let metadata = std::fs::metadata(&path)?;
    if !metadata.is_file() {
        return Err(AppError::BadRequest("path is not a file".into()));
    }
    let (bytes, total, range) = read_raw(&path, query.range.as_deref())?;
    let filename = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());
    let mut builder = Response::builder()
        .header(
            header::CONTENT_TYPE,
            crate::media::content_type_for_path(&path),
        )
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{filename}\""),
        )
        .header(header::CONTENT_LENGTH, bytes.len())
        .header(header::ACCEPT_RANGES, "bytes");
    if let Some((start, end)) = range {
        builder = builder.status(StatusCode::PARTIAL_CONTENT).header(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{total}"),
        );
    }
    builder
        .body(Body::from(bytes))
        .map_err(|error| AppError::Internal(format!("could not build response: {error}")))
}

/// Read a file, honoring an optional `bytes=start-end` range so large media
/// can be streamed by the viewer without loading it whole.
pub type RawChunk = (Vec<u8>, u64, Option<(u64, u64)>);

pub fn read_raw(path: &Path, range: Option<&str>) -> AppResult<RawChunk> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path)?;
    let total = file.metadata()?.len();
    let parsed = range.and_then(|value| {
        let value = value.trim().strip_prefix("bytes=")?;
        let (start, end) = value.split_once('-')?;
        let start = start.trim().parse::<u64>().ok()?;
        let end = if end.trim().is_empty() {
            total.saturating_sub(1)
        } else {
            end.trim().parse::<u64>().ok()?.min(total.saturating_sub(1))
        };
        (start <= end && start < total).then_some((start, end))
    });
    if let Some((start, end)) = parsed {
        file.seek(SeekFrom::Start(start))?;
        let mut bytes = vec![0; (end - start + 1) as usize];
        file.read_exact(&mut bytes)?;
        Ok((bytes, total, Some((start, end))))
    } else {
        if total > 250 * 1024 * 1024 {
            return Err(AppError::BadRequest(
                "file is too large for the in-app viewer".into(),
            ));
        }
        let mut bytes = Vec::with_capacity(total as usize);
        file.read_to_end(&mut bytes)?;
        Ok((bytes, total, None))
    }
}
