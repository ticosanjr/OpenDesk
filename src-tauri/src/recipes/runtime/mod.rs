mod calendar;
mod cron;
mod folder;
#[cfg(test)]
mod tests;

use crate::{
    app_state::AppState,
    db,
    error::AppResult,
    events::OpenDeskWsEvent,
    recipes::{engine, parser, store},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Instant, SystemTime},
};
use tokio::sync::Mutex;

use calendar::scan_calendar_triggers;
use cron::scan_cron_triggers;
use folder::scan_folder_triggers;

pub struct RecipeRuntime {
    pending: Mutex<HashMap<PathBuf, (u64, i64, Instant)>>,
    /// Retry only applies to a failure to *start* a run. A failed run keeps its
    /// fingerprint and is not retried until the file changes.
    retry_at: Mutex<HashMap<PathBuf, (Instant, String)>>,
    run_checks: Mutex<HashMap<PathBuf, String>>,
    baselines: Mutex<HashMap<String, SystemTime>>,
    started_at: SystemTime,
}

impl Default for RecipeRuntime {
    fn default() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            retry_at: Mutex::new(HashMap::new()),
            run_checks: Mutex::new(HashMap::new()),
            baselines: Mutex::new(HashMap::new()),
            started_at: SystemTime::now(),
        }
    }
}

pub fn start(state: Arc<AppState>) -> Arc<RecipeRuntime> {
    let runtime = Arc::new(RecipeRuntime::default());
    let state_clone = state.clone();
    tokio::spawn(async move {
        let _ = sync_all(&state_clone).await;
        let mut receiver = state_clone.events.subscribe();
        loop {
            if let Ok(envelope) = receiver.recv().await
                && let OpenDeskWsEvent::FsSyncTriggered {
                    workspace_id, path, ..
                } = envelope.event
                && path.contains("/recipes/")
            {
                let _ = sync_workspace(&state_clone, &workspace_id).await;
            }
        }
    });
    let runtime_clone = runtime.clone();
    tokio::spawn(async move {
        loop {
            if let Err(error) = scan_folder_triggers(&state, &runtime_clone).await {
                tracing::warn!(error = %error, "recipe folder scan failed");
            }
            if let Err(error) = scan_cron_triggers(&state).await {
                tracing::warn!(error = %error, "recipe cron scan failed");
            }
            if let Err(error) = scan_calendar_triggers(&state).await {
                tracing::warn!(error = %error, "recipe calendar scan failed");
            }
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });
    runtime
}

/// Restarts durable runs that were interrupted by a crash/restart and are not
/// waiting for a human cancellation.
pub async fn resume_runs(state: &Arc<AppState>) -> AppResult<usize> {
    let runs = store::resumable_runs(&state.db).await?;
    let mut resumed = 0;
    for run in runs {
        let Ok(yaml) = store::yaml_for(&state.db, &run.workspace_id, &run.recipe_id).await else {
            continue;
        };
        let Ok(recipe) = parser::parse(&yaml) else {
            continue;
        };
        engine::resume(crate::app_state::recipe_context(state), recipe, run).await?;
        resumed += 1;
    }
    Ok(resumed)
}

pub async fn sync_all(state: &Arc<AppState>) -> AppResult<()> {
    for workspace in db::workspaces::list(&state.db).await? {
        sync_workspace(state, &workspace.id).await?;
    }
    Ok(())
}

pub async fn sync_workspace(state: &Arc<AppState>, workspace_id: &str) -> AppResult<()> {
    let workspace = db::workspaces::get(&state.db, workspace_id).await?;
    store::sync_workspace(
        &state.db,
        workspace_id,
        std::path::Path::new(&workspace.root_path),
    )
    .await?;
    crate::api::recipes::sync_all_recipe_trigger_sources(state).await?;
    Ok(())
}
