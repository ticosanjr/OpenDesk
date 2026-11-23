use super::DropzoneConfig;
use crate::{
    app_state::AppState,
    db::io_bus::{self, InboundSource},
    error::AppResult,
    io_bus::{NormalizedTaskEvent, preprocess},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

pub(super) async fn process_with_retry(
    state: Arc<AppState>,
    source: InboundSource,
    config: DropzoneConfig,
    workspace_root: PathBuf,
    path: PathBuf,
) {
    let settle = config.settle_secs.unwrap_or(2).clamp(0, 300);
    if settle > 0 {
        tokio::time::sleep(Duration::from_secs(settle)).await;
    }
    let attempts = 2;
    let mut last_error = None;
    for attempt in 0..=attempts {
        match process_file(&state, &source, &config, &workspace_root, &path).await {
            Ok(()) => return,
            Err(error) => {
                last_error = Some(error);
                if attempt < attempts {
                    let wait = config.retry_minutes.unwrap_or(1).clamp(1, 1440) * 60;
                    tokio::time::sleep(Duration::from_secs(wait)).await;
                }
            }
        }
    }
    if let Some(error) = last_error {
        tracing::warn!(path = %path.display(), error = %error, "dropzone processing failed after retries");
        let _ = preprocess::move_failed(&path, &workspace_root);
        let _ = io_bus::mark_inbound(&state.db, &source.id, "FAILED", Some(&format!("{error:#}")))
            .await;
    }
}

pub(super) async fn process_file(
    state: &Arc<AppState>,
    source: &InboundSource,
    config: &DropzoneConfig,
    workspace_root: &Path,
    path: &Path,
) -> AppResult<()> {
    if is_ignored(path, &config.ignore_patterns) {
        return Ok(());
    }
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if metadata.len() > 100 * 1024 * 1024 {
        tracing::warn!(path = %path.display(), "dropzone file ignored: larger than 100 MB");
        return Ok(());
    }
    if metadata.len() == 0 && !config.process_empty {
        tracing::info!(path = %path.display(), "dropzone empty file ignored");
        return Ok(());
    }
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Arquivo recebido")
        .replace(['_', '-'], " ");
    let title = format!("{stem} · {}", source.name);
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
        Err(error) => return Err(error),
    };
    let source_details = format!(
        "## Origem do gatilho\n\n- Fonte: **{}** (`{}`)\n- Arquivo: `{relative}`\n- Tamanho: {} bytes\n- Pasta monitorada: `{}`\n\n",
        source.name,
        source.source_type,
        metadata.len(),
        path.parent()
            .map(|value| value.display().to_string())
            .unwrap_or_default(),
    );
    let body = format!("{source_details}{body}");
    let event = NormalizedTaskEvent {
        source_id: source.id.clone(),
        external_id: relative.clone(),
        title,
        body_markdown: body,
        priority: None,
        tags: vec![format!("file:{extension}")],
        attachments: vec![relative],
        metadata: serde_json::json!({
            "path": path,
            "size": metadata.len(),
            "source_id": source.id,
            "source_type": source.source_type,
            "source_name": source.name,
            "detected_at": chrono::Utc::now().to_rfc3339(),
        }),
    };
    match crate::io_bus::ingest(state, source, event).await {
        Ok(_) => {
            let _ = preprocess::move_processed(path, workspace_root);
            let _ = io_bus::mark_inbound(&state.db, &source.id, "TRIGGERED", None).await;
            Ok(())
        }
        Err(error) => {
            tracing::warn!(error = %error, "dropzone ingest failed");
            Err(error)
        }
    }
}

async fn audio_text(path: &Path, config: &DropzoneConfig) -> AppResult<String> {
    let model = config.whisper_model.as_deref().map(Path::new);
    preprocess::transcribe(path, model).await
}

pub(super) fn is_ignored(path: &Path, patterns: &[String]) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if name.starts_with('.')
        || name.ends_with(".tmp")
        || name.ends_with(".part")
        || name.ends_with(".crdownload")
    {
        return true;
    }
    patterns.iter().any(|pattern| {
        let pattern = pattern.trim();
        !pattern.is_empty() && name.to_lowercase().contains(&pattern.to_lowercase())
    })
}
