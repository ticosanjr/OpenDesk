use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Query, State},
};
use std::sync::Arc;

use super::paths::{FileQuery, relative_to, resolve_path, workspace_root};

/// Read technical metadata for video/audio assets with ffprobe. This gives
/// agents and the viewer duration, codecs and resolution without decoding.
pub async fn media(
    State(state): State<Arc<AppState>>,
    Query(query): Query<FileQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let root = workspace_root(&state, query.workspace_id.as_deref()).await?;
    let path = resolve_path(&root, query.path.as_deref())?;
    if !path.is_file() {
        return Err(AppError::BadRequest("path is not a file".into()));
    }
    let mut command = tokio::process::Command::new("ffprobe");
    command
        .kill_on_drop(true)
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(&path);
    #[cfg(unix)]
    command.process_group(0);
    let output = tokio::time::timeout(std::time::Duration::from_secs(20), command.output())
        .await
        .map_err(|_| AppError::Internal("ffprobe timed out".into()))?
        .map_err(|error| AppError::BadRequest(format!("ffprobe indisponível: {error}")))?;
    if !output.status.success() {
        return Err(AppError::BadRequest(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| AppError::Internal(format!("ffprobe output: {error}")))?;
    let format = value.get("format");
    let streams = value
        .get("streams")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|stream| {
                    serde_json::json!({
                        "type": stream.get("codec_type").and_then(serde_json::Value::as_str),
                        "codec": stream.get("codec_name").and_then(serde_json::Value::as_str),
                        "width": stream.get("width").and_then(serde_json::Value::as_u64),
                        "height": stream.get("height").and_then(serde_json::Value::as_u64),
                        "channels": stream.get("channels").and_then(serde_json::Value::as_u64),
                        "sample_rate": stream.get("sample_rate").and_then(serde_json::Value::as_str),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(Json(serde_json::json!({
        "path": relative_to(&root, &path),
        "format": format.and_then(|value| value.get("format_long_name")).and_then(serde_json::Value::as_str),
        "duration_secs": format.and_then(|value| value.get("duration")).and_then(serde_json::Value::as_str).and_then(|value| value.parse::<f64>().ok()),
        "bit_rate": format.and_then(|value| value.get("bit_rate")).and_then(serde_json::Value::as_str).and_then(|value| value.parse::<u64>().ok()),
        "streams": streams,
    })))
}
