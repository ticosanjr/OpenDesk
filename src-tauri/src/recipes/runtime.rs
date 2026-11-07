use crate::{
    app_state::AppState,
    db,
    error::AppResult,
    events::OpenDeskWsEvent,
    recipes::{engine, parser, store},
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct RecipeRuntime {
    seen: Mutex<HashMap<String, HashSet<PathBuf>>>,
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
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        }
    });
    runtime
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
    Ok(())
}

async fn scan_folder_triggers(
    state: &Arc<AppState>,
    runtime: &Arc<RecipeRuntime>,
) -> AppResult<()> {
    for index in store::list_all(&state.db).await? {
        if !index.is_enabled {
            continue;
        }
        let Ok(yaml) = store::yaml_for(&state.db, &index.id).await else {
            continue;
        };
        let Ok(recipe) = parser::parse(&yaml) else {
            continue;
        };
        if recipe.trigger.trigger_type != "FOLDER_WATCH"
            && recipe.trigger.trigger_type != "RSYNC_FOLDER"
        {
            continue;
        }
        let Some(path) = recipe
            .trigger
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
        for entry in entries.flatten() {
            let file = entry.path();
            if !file.is_file() {
                continue;
            }
            let mut seen = runtime.seen.lock().await;
            let set = seen.entry(recipe.id.clone()).or_default();
            if !set.insert(file.clone()) {
                continue;
            }
            drop(seen);
            let trigger = serde_json::json!({
                "file_path": file.to_string_lossy(),
                "file_name": file.file_name().and_then(|value| value.to_str()).unwrap_or("arquivo"),
                "recipe_id": recipe.id,
            });
            let _ = engine::start(state.clone(), recipe.clone(), trigger, false).await;
        }
    }
    Ok(())
}
