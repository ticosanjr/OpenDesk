use crate::{
    app_state::AppState,
    db,
    error::AppResult,
    recipes::{engine, parser, store},
};
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use super::RecipeRuntime;

#[derive(Debug, Clone)]
pub(super) struct FolderTriggerOptions {
    pub(super) settle_secs: u64,
    pub(super) process_empty: bool,
    pub(super) process_existing: bool,
    pub(super) retry_minutes: u64,
    ignore_patterns: Vec<String>,
}

impl FolderTriggerOptions {
    pub(super) fn from_config(config: &serde_json::Value) -> Self {
        let settle_secs = config
            .get("settle_secs")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(2)
            .clamp(0, 300);
        let process_empty = config
            .get("process_empty")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let process_existing = config
            .get("process_existing")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);
        let retry_minutes = config
            .get("retry_minutes")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(1)
            .clamp(1, 1440);
        let ignore_patterns = config
            .get("ignore_patterns")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            settle_secs,
            process_empty,
            process_existing,
            retry_minutes,
            ignore_patterns,
        }
    }

    pub(super) fn ignores(&self, name: &str) -> bool {
        name.starts_with('.')
            || name.ends_with(".tmp")
            || name.ends_with(".part")
            || name.ends_with(".crdownload")
            || self.ignore_patterns.iter().any(|pattern| {
                !pattern.trim().is_empty()
                    && name.to_lowercase().contains(&pattern.trim().to_lowercase())
            })
    }
}

pub(super) async fn release_failed_runs(state: &Arc<AppState>, runtime: &Arc<RecipeRuntime>) {
    let checks = runtime
        .run_checks
        .lock()
        .await
        .iter()
        .map(|(path, run_id)| (path.clone(), run_id.clone()))
        .collect::<Vec<_>>();
    for (path, run_id) in checks {
        let status = sqlx::query_scalar::<_, String>("SELECT status FROM recipe_runs WHERE id = ?")
            .bind(&run_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();
        let Some(status) = status else {
            runtime.run_checks.lock().await.remove(&path);
            continue;
        };
        let terminal = match status.as_str() {
            "COMPLETED" => Some("COMPLETED"),
            "FAILED" | "REJECTED" | "TIMED_OUT" => Some("FAILED"),
            _ => None,
        };
        if let Some(terminal) = terminal {
            let _ = db::recipe_files::update_status_by_run(&state.db, &run_id, terminal).await;
            runtime.run_checks.lock().await.remove(&path);
        }
    }
}

pub(super) async fn scan_folder_triggers(
    state: &Arc<AppState>,
    runtime: &Arc<RecipeRuntime>,
) -> AppResult<()> {
    release_failed_runs(state, runtime).await;
    for index in store::list_all(&state.db).await? {
        if !index.is_enabled {
            continue;
        }
        let Ok(yaml) = store::yaml_for(&state.db, &index.workspace_id, &index.id).await else {
            continue;
        };
        let Ok(recipe) = parser::parse(&yaml) else {
            continue;
        };
        // Folder/RSYNC triggers are materialized as io_bus sources and handled
        // by the dropzone worker. The scanner stays as a fallback for recipes
        // whose sources were not synced yet.
        if crate::db::io_bus::list_inbound(&state.db, &index.workspace_id)
            .await
            .unwrap_or_default()
            .iter()
            .any(|source| {
                source.source_type == "FOLDER_WATCH"
                    && serde_json::from_str::<serde_json::Value>(&source.config_json)
                        .ok()
                        .and_then(|config| {
                            (config.get("recipe_id").and_then(|value| value.as_str())
                                == Some(index.id.as_str()))
                            .then_some(())
                        })
                        .is_some()
            })
        {
            continue;
        }
        let folder_triggers = recipe
            .triggers()
            .into_iter()
            .filter(|trigger| {
                trigger.trigger_type == "FOLDER_WATCH" || trigger.trigger_type == "RSYNC_FOLDER"
            })
            .cloned()
            .collect::<Vec<_>>();
        if folder_triggers.is_empty() {
            continue;
        }
        for folder_trigger in folder_triggers {
            let Some(path) = folder_trigger
                .config
                .get("path")
                .and_then(|value| value.as_str())
            else {
                continue;
            };
            let workspace = db::workspaces::get(&state.db, &index.workspace_id).await?;
            let dir = PathBuf::from(&workspace.root_path).join(path);
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            let options = FolderTriggerOptions::from_config(&folder_trigger.config);
            let settle_secs = options.settle_secs;
            let process_empty = options.process_empty;
            let process_existing = options.process_existing;
            let retry_minutes = options.retry_minutes;
            let baseline_key = format!("{}:{path}", recipe.id);
            let baseline = if process_existing {
                runtime.started_at
            } else {
                *runtime
                    .baselines
                    .lock()
                    .await
                    .entry(baseline_key)
                    .or_insert_with(SystemTime::now)
            };
            let known_files =
                db::recipe_files::list_for_trigger(&state.db, &recipe.id, path).await?;
            // First scan after upgrading from the in-memory implementation:
            // files already present in recipe_runs should not run again just
            // because recipe_file_state did not exist yet.
            let mut legacy_processed = HashSet::new();
            if known_files.is_empty()
                && let Ok(previous) =
                    db::recipe_files::latest_run_files(&state.db, &recipe.id).await
            {
                for (legacy_path, started_at) in previous {
                    let file = PathBuf::from(&legacy_path);
                    let (Ok(metadata), Ok(started_at)) = (
                        std::fs::metadata(&file),
                        chrono::DateTime::parse_from_rfc3339(&started_at),
                    ) else {
                        continue;
                    };
                    let modified = metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                    let started_at: SystemTime = started_at.with_timezone(&chrono::Utc).into();
                    if modified <= started_at {
                        let _ = db::recipe_files::upsert(
                            &state.db,
                            &recipe.id,
                            path,
                            &legacy_path,
                            &file_fingerprint(&metadata),
                            "SEEN",
                            None,
                        )
                        .await;
                        legacy_processed.insert(legacy_path);
                    }
                }
            }

            for entry in entries.flatten() {
                let file = entry.path();
                if !file.is_file() {
                    continue;
                }
                let Ok(metadata) = std::fs::metadata(&file) else {
                    continue;
                };
                let name = file
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("arquivo");
                if options.ignores(name) {
                    continue;
                }
                if metadata.len() == 0 && !process_empty {
                    continue;
                }
                let fingerprint = file_fingerprint(&metadata);
                let file_key = file.to_string_lossy().to_string();
                if legacy_processed.contains(&file_key)
                    || already_processed(known_files.get(&file_key), &fingerprint)
                {
                    continue;
                }
                if !process_existing
                    && metadata
                        .modified()
                        .map(|value| value < baseline)
                        .unwrap_or(false)
                {
                    let _ = db::recipe_files::upsert(
                        &state.db,
                        &recipe.id,
                        path,
                        &file_key,
                        &fingerprint,
                        "SEEN",
                        None,
                    )
                    .await;
                    continue;
                }
                {
                    let retry = runtime.retry_at.lock().await;
                    if let Some((at, retry_fingerprint)) = retry.get(&file)
                        && *at > Instant::now()
                        && retry_fingerprint == &fingerprint
                    {
                        continue;
                    }
                }
                if runtime.run_checks.lock().await.contains_key(&file) {
                    continue;
                }
                if settle_secs > 0 {
                    let modified = metadata
                        .modified()
                        .ok()
                        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|value| value.as_secs() as i64)
                        .unwrap_or(0);
                    let mut pending = runtime.pending.lock().await;
                    match pending.get(&file) {
                        Some((size, stamp, since))
                            if *size == metadata.len()
                                && *stamp == modified
                                && since.elapsed() >= Duration::from_secs(settle_secs) =>
                        {
                            pending.remove(&file);
                        }
                        Some((size, stamp, _)) if *size == metadata.len() && *stamp == modified => {
                            continue;
                        }
                        _ => {
                            pending
                                .insert(file.clone(), (metadata.len(), modified, Instant::now()));
                            continue;
                        }
                    }
                }
                let trigger = serde_json::json!({
                    "file_path": file.to_string_lossy(),
                    "file_name": name,
                    "file_size": metadata.len(),
                    "modified_at": metadata.modified().ok().map(|value| chrono::DateTime::<chrono::Utc>::from(value).to_rfc3339()),
                    "trigger_type": folder_trigger.trigger_type,
                    "trigger_path": path,
                    "recipe_id": recipe.id,
                });
                match engine::start(
                    crate::app_state::recipe_context(state),
                    &index.workspace_id,
                    recipe.clone(),
                    trigger,
                    false,
                )
                .await
                {
                    Ok(run) => {
                        if let Err(error) = db::recipe_files::upsert(
                            &state.db,
                            &recipe.id,
                            path,
                            &file_key,
                            &fingerprint,
                            "RUNNING",
                            Some(&run.id),
                        )
                        .await
                        {
                            tracing::warn!(path = %file.display(), error = %error, "could not persist recipe file state");
                        }
                        runtime.run_checks.lock().await.insert(file.clone(), run.id);
                    }
                    Err(error) => {
                        tracing::warn!(path = %file.display(), error = %error, "recipe folder trigger failed; will retry");
                        let mut retry = runtime.retry_at.lock().await;
                        retry.insert(
                            file.clone(),
                            (
                                Instant::now() + Duration::from_secs(retry_minutes * 60),
                                fingerprint,
                            ),
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn already_processed(
    known: Option<&db::recipe_files::RecipeFileState>,
    fingerprint: &str,
) -> bool {
    known.is_some_and(|known| known.fingerprint == fingerprint)
}

pub(super) fn file_fingerprint(metadata: &std::fs::Metadata) -> String {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    format!("{modified}:{}", metadata.len())
}
