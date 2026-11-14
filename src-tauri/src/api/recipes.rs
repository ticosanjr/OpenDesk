use crate::{
    app_state::AppState,
    db::io_bus::{self, InboundInput},
    error::{AppError, AppResult},
    recipes::{engine, parser, runtime, store},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct RecipeQuery {
    pub workspace_id: Option<String>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RecipeQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(serde_json::json!({
        "recipes": store::list(&state.db, &workspace_id).await?,
    })))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<RecipeQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let index = store::get(&state.db, &workspace_id, &id).await?;
    let yaml = store::yaml_for(&state.db, &workspace_id, &id).await?;
    Ok(Json(serde_json::json!({ "recipe": index, "yaml": yaml })))
}

#[derive(Debug, Deserialize)]
pub struct SaveBody {
    pub yaml: String,
}

pub async fn save(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<RecipeQuery>,
    Json(body): Json<SaveBody>,
) -> AppResult<Json<store::RecipeIndex>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let saved = store::save_yaml(&state.db, &workspace_id, &id, &body.yaml).await?;
    sync_recipe_trigger_sources(&state, &workspace_id, &id).await?;
    Ok(Json(saved))
}

#[derive(Debug, Deserialize)]
pub struct EnabledBody {
    pub enabled: bool,
}

pub async fn set_enabled(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<RecipeQuery>,
    Json(body): Json<EnabledBody>,
) -> AppResult<Json<store::RecipeIndex>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        store::set_enabled(&state.db, &workspace_id, &id, body.enabled).await?,
    ))
}

#[derive(Debug, Deserialize, Default)]
pub struct RunBody {
    #[serde(default)]
    pub trigger_context: serde_json::Value,
    #[serde(default)]
    pub dry_run: bool,
}

pub async fn run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<RecipeQuery>,
    Json(body): Json<RunBody>,
) -> AppResult<Json<store::RecipeRun>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let _index = store::get(&state.db, &workspace_id, &id).await?;
    let yaml = store::yaml_for(&state.db, &workspace_id, &id).await?;
    let recipe = parser::parse(&yaml)?;
    let trigger = if body.trigger_context.is_null() {
        serde_json::json!({ "manual": true })
    } else {
        body.trigger_context
    };
    Ok(Json(
        engine::start(state.clone(), &workspace_id, recipe, trigger, body.dry_run).await?,
    ))
}

pub async fn runs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RecipeQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(serde_json::json!({
        "runs": store::list_runs(&state.db, &workspace_id, 100).await?,
    })))
}

pub async fn delete_recipe(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<RecipeQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let index = store::get(&state.db, &workspace_id, &id).await?;
    let active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM recipe_runs
         WHERE recipe_id = ? AND workspace_id = ?
           AND status IN ('RUNNING_STEP', 'WAITING_HITL', 'RUNNING_SINKS')",
    )
    .bind(&id)
    .bind(&workspace_id)
    .fetch_one(&state.db)
    .await?;
    if active > 0 {
        return Err(AppError::Conflict(
            "esta receita tem execuções em andamento; pare-as antes de excluir".into(),
        ));
    }
    match std::fs::remove_file(&index.file_path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(AppError::Io(error));
        }
    }
    crate::db::recipe_files::delete_for_recipe(&state.db, &id).await?;
    crate::recipes::runtime::sync_workspace(&state, &workspace_id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn run_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let run = store::get_run(&state.db, &id).await?;
    let hitl = store::pending_hitl_for_run(&state.db, &id).await?;
    Ok(Json(serde_json::json!({ "run": run, "hitl": hitl })))
}

#[derive(Debug, Deserialize, Default)]
pub struct HitlQuery {
    pub workspace_id: Option<String>,
}

pub async fn hitl_pending(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HitlQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(serde_json::json!({
        "requests": store::pending_hitl(&state.db, &workspace_id).await?,
    })))
}

#[derive(Debug, Deserialize)]
pub struct ResolveBody {
    pub resolution: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub option_index: Option<usize>,
    #[serde(default)]
    pub file_path: Option<String>,
}

pub async fn resolve_hitl(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    Json(body): Json<ResolveBody>,
) -> AppResult<Json<serde_json::Value>> {
    let request = store::get_hitl(&state.db, &token).await?;
    let (kind, payload) = match body.resolution.to_uppercase().as_str() {
        "APPROVED" => ("APPROVED".to_string(), None),
        "REJECTED" => ("REJECTED".to_string(), body.text),
        "CHOICE" => (
            "CHOICE".to_string(),
            Some(
                serde_json::json!({
                    "option_index": body.option_index.unwrap_or(0),
                    "option_value": body.text.unwrap_or_default(),
                })
                .to_string(),
            ),
        ),
        "ARTIFACT" => (
            "ARTIFACT".to_string(),
            Some(serde_json::json!({ "path": body.file_path.unwrap_or_default() }).to_string()),
        ),
        _ => (
            "FEEDBACK".to_string(),
            Some(
                serde_json::json!({
                    "feedback_text": body.text.unwrap_or_default(),
                    "attached_files": body.file_path.map(|path| vec![path]).unwrap_or_default(),
                })
                .to_string(),
            ),
        ),
    };
    let resolved = crate::recipes::hitl::resolve(
        &state,
        &token,
        "DESKTOP_UI",
        None,
        &kind,
        payload.as_deref(),
    )
    .await?;
    if resolved.is_none() {
        return Err(AppError::Conflict(
            "checkpoint já resolvido ou expirado".into(),
        ));
    }
    Ok(Json(serde_json::json!({
        "ok": true,
        "run_id": request.recipe_run_id,
        "resolution": kind,
    })))
}

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
        desired.push((
            source_type.to_string(),
            format!("Receita: {} · {}", recipe.name, trigger.trigger_type),
            serde_json::Value::Object(config),
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

/// Boot/manual sync: recreate sources for every recipe and remove orphans.
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
