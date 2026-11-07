use crate::{
    app_state::AppState,
    db::io_bus::{self, InboundSource},
    error::{AppError, AppResult},
    io_bus::{NormalizedTaskEvent, preprocess, source_config},
};
use notify_debouncer_full::{
    DebounceEventResult, Debouncer, RecommendedCache, new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
};
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Default, Deserialize)]
pub struct DropzoneConfig {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub processor: Option<String>,
    #[serde(default)]
    pub languages: Option<String>,
    #[serde(default)]
    pub whisper_model: Option<String>,
}

pub async fn run(
    state: Arc<AppState>,
    source: InboundSource,
    cancel: CancellationToken,
) -> AppResult<()> {
    let config: DropzoneConfig = source_config(&source)?;
    let workspace = crate::db::workspaces::get(&state.db, &source.workspace_id).await?;
    let root = PathBuf::from(&workspace.root_path);
    let dropzone = config
        .path
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("input").join("dropzone"));
    if !dropzone.is_absolute() {
        // Relative paths are always under the workspace input directory.
        let relative = config.path.as_deref().unwrap_or("input/dropzone");
        let candidate = root.join(relative);
        std::fs::create_dir_all(&candidate)?;
        return watch(state, source, config, root, candidate, cancel).await;
    }
    std::fs::create_dir_all(&dropzone)?;
    watch(state, source, config, root, dropzone, cancel).await
}

async fn watch(
    state: Arc<AppState>,
    source: InboundSource,
    config: DropzoneConfig,
    workspace_root: PathBuf,
    dropzone: PathBuf,
    cancel: CancellationToken,
) -> AppResult<()> {
    let (tx, mut rx) = mpsc::unbounded_channel::<PathBuf>();
    let mut debouncer: Debouncer<RecommendedWatcher, RecommendedCache> = new_debouncer(
        Duration::from_millis(500),
        None,
        move |result: DebounceEventResult| {
            if let Ok(events) = result {
                for event in events {
                    for path in event.event.paths {
                        let _ = tx.send(path);
                    }
                }
            }
        },
    )
    .map_err(|error| AppError::Internal(format!("dropzone watcher: {error}")))?;
    debouncer
        .watch(&dropzone, RecursiveMode::Recursive)
        .map_err(|error| AppError::Internal(format!("dropzone watch: {error}")))?;

    // Process files already present when the source starts.
    if let Ok(entries) = std::fs::read_dir(&dropzone) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                process_file(&state, &source, &config, &workspace_root, &path).await;
            }
        }
    }

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            path = rx.recv() => {
                let Some(path) = path else { break; };
                if path.is_file() && !is_ignored(&path) {
                    process_file(&state, &source, &config, &workspace_root, &path).await;
                }
            }
        }
    }
    Ok(())
}

async fn process_file(
    state: &Arc<AppState>,
    source: &InboundSource,
    config: &DropzoneConfig,
    workspace_root: &Path,
    path: &Path,
) {
    if is_ignored(path) {
        return;
    }
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => return,
    };
    if metadata.len() > 100 * 1024 * 1024 {
        tracing::warn!(path = %path.display(), "dropzone file ignored: larger than 100 MB");
        return;
    }
    let title = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Arquivo recebido")
        .replace(['_', '-'], " ");
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let processor = config
        .processor
        .clone()
        .unwrap_or_else(|| source.source_type.clone());
    let result = match processor.as_str() {
        "AUDIO_STT_WATCH" => audio_text(path, config).await,
        "OCR_WATCH" => {
            preprocess::ocr(path, config.languages.as_deref().unwrap_or("por+eng")).await
        }
        _ => match extension.as_str() {
            "png" | "jpg" | "jpeg" | "tiff" | "webp" => {
                preprocess::ocr(path, config.languages.as_deref().unwrap_or("por+eng")).await
            }
            "wav" | "mp3" | "ogg" | "m4a" | "flac" => audio_text(path, config).await,
            "pdf" => match preprocess::extract_document(path) {
                Ok(document) if document.text.trim().len() > 20 => Ok(document.text),
                _ => preprocess::ocr(path, config.languages.as_deref().unwrap_or("por+eng")).await,
            },
            _ => preprocess::extract_document(path).map(|document| document.text),
        },
    };
    let relative = path
        .strip_prefix(workspace_root)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string();
    let body = match result {
        Ok(text) => {
            if text.trim().is_empty() {
                format!("Arquivo `{relative}` recebido sem texto extraído.")
            } else {
                format!("# {title}\n\n{text}")
            }
        }
        Err(error) => {
            tracing::warn!(path = %path.display(), error = %error, "dropzone processing failed");
            let _ = preprocess::move_failed(path, workspace_root);
            let _ =
                io_bus::mark_inbound(&state.db, &source.id, "FAILED", Some(&format!("{error:#}")))
                    .await;
            return;
        }
    };
    let event = NormalizedTaskEvent {
        source_id: source.id.clone(),
        external_id: relative.clone(),
        title,
        body_markdown: body,
        priority: None,
        tags: vec![format!("file:{extension}")],
        attachments: vec![relative],
        metadata: serde_json::json!({ "path": path, "size": metadata.len() }),
    };
    match crate::io_bus::ingest(state, source, event).await {
        Ok(_) => {
            let _ = preprocess::move_processed(path, workspace_root);
            let _ = io_bus::mark_inbound(&state.db, &source.id, "TRIGGERED", None).await;
        }
        Err(error) => {
            tracing::warn!(error = %error, "dropzone ingest failed");
            let _ = preprocess::move_failed(path, workspace_root);
            let _ =
                io_bus::mark_inbound(&state.db, &source.id, "FAILED", Some(&format!("{error:#}")))
                    .await;
        }
    }
}

async fn audio_text(path: &Path, config: &DropzoneConfig) -> AppResult<String> {
    let model = config.whisper_model.as_deref().map(Path::new);
    preprocess::transcribe(path, model).await
}

fn is_ignored(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    name.starts_with('.')
        || name.ends_with(".tmp")
        || name.ends_with(".part")
        || name.ends_with(".crdownload")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_temporary_and_hidden_files() {
        assert!(is_ignored(Path::new("/tmp/.hidden")));
        assert!(is_ignored(Path::new("/tmp/a.tmp")));
        assert!(!is_ignored(Path::new("/tmp/report.pdf")));
    }
}
