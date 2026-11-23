use super::{DropzoneConfig, process, rsync};
use crate::{
    app_state::AppState,
    db::io_bus::InboundSource,
    error::{AppError, AppResult},
};
use notify_debouncer_full::{
    DebounceEventResult, Debouncer, RecommendedCache, new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
};
use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{Semaphore, mpsc};
use tokio_util::sync::CancellationToken;

pub(super) async fn watch(
    state: Arc<AppState>,
    source: InboundSource,
    config: DropzoneConfig,
    workspace_root: PathBuf,
    dropzone: PathBuf,
    cancel: CancellationToken,
) -> AppResult<()> {
    if config.transport.as_deref() == Some("rsync")
        && let Some(remote) = config.rsync_source.clone()
    {
        rsync::spawn(
            state.clone(),
            config.clone(),
            dropzone.clone(),
            cancel.clone(),
            remote,
        );
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
                if path.is_file() && !process::is_ignored(&path, &config.ignore_patterns) {
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
        process::process_with_retry(state, source, config, workspace_root, path.clone()).await;
        in_flight.lock().await.remove(&path);
    });
}
