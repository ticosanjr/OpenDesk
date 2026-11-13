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
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{Semaphore, mpsc};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DropzoneConfig {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub processor: Option<String>,
    #[serde(default)]
    pub languages: Option<String>,
    #[serde(default)]
    pub whisper_model: Option<String>,
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(default)]
    pub rsync_source: Option<String>,
    #[serde(default)]
    pub interval_secs: Option<u64>,
    #[serde(default)]
    pub ssh_key_ref: Option<String>,
    #[serde(default)]
    pub ssh_key_path: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub delete: bool,
    #[serde(default)]
    pub checksum: bool,
    /// Wait before processing so a half-written file is not read.
    #[serde(default)]
    pub settle_secs: Option<u64>,
    /// Include files with zero bytes in the task.
    #[serde(default)]
    pub process_empty: bool,
    /// Process files that already existed when the watcher started.
    #[serde(default)]
    pub process_existing: Option<bool>,
    /// Delay before retrying a failed file.
    #[serde(default)]
    pub retry_minutes: Option<u64>,
    /// Simple substring/extension patterns ignored by the watcher.
    #[serde(default)]
    pub ignore_patterns: Vec<String>,
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
    if config.transport.as_deref() == Some("rsync")
        && let Some(source) = config.rsync_source.clone()
    {
        let state = state.clone();
        let config = config.clone();
        let dropzone = dropzone.clone();
        let cancel = cancel.clone();
        tokio::spawn(async move {
            let key_path = if let Some(key) = config.ssh_key_ref.as_deref() {
                crate::secrets::SecretStore::new(&state.paths)
                    .get("io_bus", key)
                    .ok()
                    .flatten()
            } else {
                config.ssh_key_path.clone()
            };
            let rsync_config = crate::io_bus::outbound::rsync::RsyncConfig {
                ssh_key_path: key_path,
                port: config.port,
                delete: config.delete,
                checksum: config.checksum,
                timeout_secs: Some(120),
                extra_args: Vec::new(),
            };
            loop {
                if cancel.is_cancelled() {
                    break;
                }
                let _ =
                    crate::io_bus::outbound::rsync::pull(&source, &dropzone, &rsync_config).await;
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = tokio::time::sleep(Duration::from_secs(config.interval_secs.unwrap_or(300).clamp(15, 3600))) => {}
                }
            }
        });
    }

    let in_flight: Arc<tokio::sync::Mutex<HashSet<PathBuf>>> =
        Arc::new(tokio::sync::Mutex::new(HashSet::new()));
    let limit = Arc::new(Semaphore::new(4));
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

    // Process files already present when the source starts, unless disabled.
    if config.process_existing.unwrap_or(true)
        && let Ok(entries) = std::fs::read_dir(&dropzone)
    {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                schedule_processing(
                    state.clone(),
                    source.clone(),
                    config.clone(),
                    workspace_root.clone(),
                    path,
                    in_flight.clone(),
                    limit.clone(),
                )
                .await;
            }
        }
    }

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            path = rx.recv() => {
                let Some(path) = path else { break; };
                if path.is_file() && !is_ignored(&path, &config.ignore_patterns) {
                    schedule_processing(
                        state.clone(),
                        source.clone(),
                        config.clone(),
                        workspace_root.clone(),
                        path,
                        in_flight.clone(),
                        limit.clone(),
                    )
                    .await;
                }
            }
        }
    }
    Ok(())
}

/// Avoid the duplicate create/modify events the debouncer can still emit for
/// the same file, and bound heavy OCR/audio work.
async fn schedule_processing(
    state: Arc<AppState>,
    source: InboundSource,
    config: DropzoneConfig,
    workspace_root: PathBuf,
    path: PathBuf,
    in_flight: Arc<tokio::sync::Mutex<HashSet<PathBuf>>>,
    limit: Arc<Semaphore>,
) {
    if !in_flight.lock().await.insert(path.clone()) {
        return;
    }
    tokio::spawn(async move {
        let _permit = limit.acquire_owned().await.ok();
        process_with_retry(state, source, config, workspace_root, path.clone()).await;
        in_flight.lock().await.remove(&path);
    });
}

async fn process_with_retry(
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

async fn process_file(
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

fn is_ignored(path: &Path, patterns: &[String]) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn source(workspace_id: &str) -> InboundSource {
        InboundSource {
            id: "source-1".into(),
            workspace_id: workspace_id.into(),
            name: "Pasta de teste".into(),
            source_type: "FOLDER_WATCH".into(),
            config_json: "{}".into(),
            target_agent: None,
            target_column: "TODO".into(),
            auto_execute: false,
            is_enabled: true,
            last_triggered_at: None,
            last_status: None,
            last_error: None,
            created_at: "2026-11-13T00:00:00Z".into(),
        }
    }

    #[tokio::test]
    async fn processes_a_file_into_a_durable_task() {
        let ctx = crate::test_support::context().await;
        let dir = ctx.workspace_root.join("input/dropzone");
        std::fs::create_dir_all(&dir).expect("dir");
        let file = dir.join("relatorio.txt");
        std::fs::write(&file, "conteudo do relatorio").expect("file");
        let source = source(&ctx.workspace_id);
        let config = DropzoneConfig::default();

        process_file(&ctx.state, &source, &config, &ctx.workspace_root, &file)
            .await
            .expect("process");

        let tasks = crate::db::tasks::list(&ctx.state.db, &ctx.workspace_id)
            .await
            .expect("tasks");
        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].title.contains("Pasta de teste"));
        assert!(tasks[0].description.contains("Origem do gatilho"));
        assert!(!file.exists());
    }

    #[tokio::test]
    async fn retry_wrapper_processes_a_file() {
        let ctx = crate::test_support::context().await;
        let dir = ctx.workspace_root.join("input/dropzone");
        std::fs::create_dir_all(&dir).expect("dir");
        let file = dir.join("retry.txt");
        std::fs::write(&file, "conteudo").expect("file");
        let source = source(&ctx.workspace_id);
        let config = DropzoneConfig {
            settle_secs: Some(0),
            ..DropzoneConfig::default()
        };

        process_with_retry(
            ctx.state.clone(),
            source,
            config,
            ctx.workspace_root.clone(),
            file.clone(),
        )
        .await;

        assert!(!file.exists());
        let tasks = crate::db::tasks::list(&ctx.state.db, &ctx.workspace_id)
            .await
            .expect("tasks");
        assert_eq!(tasks.len(), 1);
    }

    #[tokio::test]
    async fn empty_files_are_skipped_by_default() {
        let ctx = crate::test_support::context().await;
        let dir = ctx.workspace_root.join("input/dropzone");
        std::fs::create_dir_all(&dir).expect("dir");
        let file = dir.join("vazio.txt");
        std::fs::write(&file, b"").expect("file");
        let source = source(&ctx.workspace_id);
        let config = DropzoneConfig::default();

        process_file(&ctx.state, &source, &config, &ctx.workspace_root, &file)
            .await
            .expect("process");
        assert!(file.exists());
        let tasks = crate::db::tasks::list(&ctx.state.db, &ctx.workspace_id)
            .await
            .expect("tasks");
        assert!(tasks.is_empty());
    }

    #[test]
    fn parses_the_folder_watch_options() {
        let config: DropzoneConfig = serde_json::from_value(serde_json::json!({
            "path": "input/dropzone",
            "settle_secs": 5,
            "process_empty": true,
            "process_existing": false,
            "retry_minutes": 15,
            "ignore_patterns": ["tmp", "backup"]
        }))
        .expect("config");
        assert_eq!(config.settle_secs, Some(5));
        assert!(config.process_empty);
        assert_eq!(config.process_existing, Some(false));
        assert_eq!(config.retry_minutes, Some(15));
        assert_eq!(config.ignore_patterns, vec!["tmp", "backup"]);
    }

    #[test]
    fn ignores_temporary_and_hidden_files() {
        assert!(is_ignored(Path::new("/tmp/.hidden"), &[]));
        assert!(is_ignored(Path::new("/tmp/a.tmp"), &[]));
        assert!(!is_ignored(Path::new("/tmp/report.pdf"), &[]));
        assert!(is_ignored(
            Path::new("/tmp/ignorar.pdf"),
            &["ignorar".into()]
        ));
    }
}
