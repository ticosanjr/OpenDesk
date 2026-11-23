use super::RecipeQuery;
use crate::{
    app_state::AppState,
    db::io_bus::{self, InboundInput},
    error::AppResult,
    recipes::{parser, runtime, store},
};
use axum::{
    Json,
    extract::{Query, State},
};
use std::sync::Arc;

pub async fn sync(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RecipeQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    runtime::sync_workspace(&state, &workspace_id).await?;
    sync_all_recipe_trigger_sources(&state).await?;
    Ok(Json(serde_json::json!({
        "recipes": store::list(&state.db, &workspace_id).await?,
    })))
}

/// Input sources are how IMAP/Webhook/SQL recipe triggers reach the engine.
/// Folder and cron triggers are handled by the recipe runtime itself.
fn recipe_source_type(trigger_type: &str) -> Option<&'static str> {
    match trigger_type {
        "IMAP_IDLE" => Some("IMAP_IDLE"),
        "WEBHOOK" => Some("WEBHOOK_HTTP"),
        "SQL_POLL" => Some("SQL_POLLER"),
        _ => None,
    }
}

async fn sync_recipe_sources_inner(
    state: &Arc<AppState>,
    workspace_id: &str,
    recipe_id: &str,
) -> AppResult<bool> {
    let yaml = store::yaml_for(&state.db, workspace_id, recipe_id).await?;
    let recipe = parser::parse(&yaml)?;
    let mut desired: Vec<(String, String, serde_json::Value)> = Vec::new();
    for (index, trigger) in recipe.triggers().into_iter().enumerate() {
        let Some(source_type) = recipe_source_type(&trigger.trigger_type) else {
            continue;
        };
        let mut config = trigger.config.as_object().cloned().unwrap_or_default();
        config.insert("recipe_id".into(), serde_json::json!(recipe.id));
        config.insert("trigger_index".into(), serde_json::json!(index));
        config.insert(
            "trigger_type".into(),
            serde_json::json!(trigger.trigger_type),
        );
        let mut config = serde_json::Value::Object(config);
        if source_type == "WEBHOOK_HTTP" {
            crate::io_bus::ensure_webhook_secret_with(
                &state.paths,
                &mut config,
                &format!("recipe-{}-{}", recipe.id, index),
            )?;
        }
        desired.push((
            source_type.to_string(),
            format!("Receita: {} · {}", recipe.name, trigger.trigger_type),
            config,
        ));
    }

    let existing = io_bus::list_inbound(&state.db, workspace_id).await?;
    let mut kept = Vec::new();
    let mut changed = false;
    for (source_type, name, config) in desired {
        let trigger_index = config
            .get("trigger_index")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        let found = existing.iter().find(|source| {
            serde_json::from_str::<serde_json::Value>(&source.config_json)
                .ok()
                .and_then(|value| {
                    let same_recipe = value.get("recipe_id").and_then(serde_json::Value::as_str)
                        == Some(recipe.id.as_str());
                    let same_index = value
                        .get("trigger_index")
                        .and_then(serde_json::Value::as_u64)
                        == Some(trigger_index);
                    let same_type = value
                        .get("trigger_type")
                        .and_then(serde_json::Value::as_str)
                        == config
                            .get("trigger_type")
                            .and_then(serde_json::Value::as_str);
                    (same_recipe && same_index && same_type).then_some(true)
                })
                .unwrap_or(false)
        });
        let input = InboundInput {
            name,
            source_type,
            config,
            target_agent: None,
            target_column: Some("TODO".into()),
            auto_execute: true,
            is_enabled: recipe.enabled,
        };
        crate::io_bus::secrets_guard::validate_no_plaintext_secrets(&input.config)?;
        match found {
            Some(source) => {
                io_bus::update_inbound(&state.db, &source.id, input).await?;
                kept.push(source.id.clone());
                changed = true;
            }
            None => {
                let source = io_bus::create_inbound(&state.db, workspace_id, input).await?;
                kept.push(source.id);
                changed = true;
            }
        }
    }

    for source in existing {
        if kept.contains(&source.id) {
            continue;
        }
        let linked = serde_json::from_str::<serde_json::Value>(&source.config_json)
            .ok()
            .and_then(|value| {
                let managed = value.get("trigger_index").is_some();
                let same_recipe = value.get("recipe_id").and_then(serde_json::Value::as_str)
                    == Some(recipe.id.as_str());
                (managed && same_recipe).then_some(true)
            })
            .unwrap_or(false);
        if linked {
            io_bus::delete_inbound(&state.db, &source.id).await?;
            changed = true;
        }
    }
    Ok(changed)
}

pub(crate) async fn sync_recipe_trigger_sources(
    state: &Arc<AppState>,
    workspace_id: &str,
    recipe_id: &str,
) -> AppResult<()> {
    if sync_recipe_sources_inner(state, workspace_id, recipe_id).await? {
        state.io_bus.reload(state.clone()).await?;
    }
    Ok(())
}

pub async fn sync_all_recipe_trigger_sources(state: &Arc<AppState>) -> AppResult<()> {
    let mut changed = false;
    let mut known = std::collections::HashSet::new();
    for index in store::list_all(&state.db).await? {
        match sync_recipe_sources_inner(state, &index.workspace_id, &index.id).await {
            Ok(did_change) => {
                known.insert((index.workspace_id.clone(), index.id.clone()));
                if did_change {
                    changed = true;
                }
            }
            Err(error) => {
                // A recipe file may have been removed by an uninstall while its
                // index row is still waiting for the watcher. Prune that
                // workspace and keep syncing the rest instead of failing the
                // whole boot/install.
                tracing::warn!(
                    workspace_id = %index.workspace_id,
                    recipe_id = %index.id,
                    error = %error,
                    "skipping unreadable recipe during source sync"
                );
                if let Ok(workspace) =
                    crate::db::workspaces::get(&state.db, &index.workspace_id).await
                {
                    let _ = store::sync_workspace(
                        &state.db,
                        &index.workspace_id,
                        std::path::Path::new(&workspace.root_path),
                    )
                    .await;
                }
            }
        }
    }
    // Sources linked to recipes that no longer exist are removed.
    for workspace in crate::db::workspaces::list(&state.db).await? {
        for source in io_bus::list_inbound(&state.db, &workspace.id).await? {
            let recipe_id = serde_json::from_str::<serde_json::Value>(&source.config_json)
                .ok()
                .and_then(|value| {
                    value.get("trigger_index")?;
                    value
                        .get("recipe_id")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                });
            if let Some(recipe_id) = recipe_id
                && !known.contains(&(workspace.id.clone(), recipe_id))
            {
                io_bus::delete_inbound(&state.db, &source.id).await?;
                changed = true;
            }
        }
    }
    if changed {
        state.io_bus.reload(state.clone()).await?;
    }
    Ok(())
}
