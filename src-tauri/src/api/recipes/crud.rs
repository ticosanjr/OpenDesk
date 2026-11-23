use super::{RecipeQuery, triggers::sync_recipe_trigger_sources};
use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    recipes::{runtime, store},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

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
    runtime::sync_workspace(&state, &workspace_id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Debug, Deserialize, Default)]
pub struct VersionQuery {
    pub workspace_id: Option<String>,
    pub against: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RollbackBody {
    pub version_id: String,
}

pub async fn versions(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<RecipeQuery>,
) -> AppResult<Json<Vec<store::RecipeVersionSummary>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        store::list_versions(&state.db, &workspace_id, &id).await?,
    ))
}

pub async fn version(
    State(state): State<Arc<AppState>>,
    Path((id, version_id)): Path<(String, String)>,
    Query(query): Query<RecipeQuery>,
) -> AppResult<Json<store::RecipeVersion>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        store::get_version(&state.db, &workspace_id, &id, &version_id).await?,
    ))
}

pub async fn version_diff(
    State(state): State<Arc<AppState>>,
    Path((id, version_id)): Path<(String, String)>,
    Query(query): Query<VersionQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let diff = store::diff_version(
        &state.db,
        &workspace_id,
        &id,
        &version_id,
        query.against.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "diff": diff })))
}

pub async fn rollback(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<RecipeQuery>,
    Json(body): Json<RollbackBody>,
) -> AppResult<Json<store::RecipeIndex>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let index = store::rollback_version(&state.db, &workspace_id, &id, &body.version_id).await?;
    sync_recipe_trigger_sources(&state, &workspace_id, &id).await?;
    Ok(Json(index))
}
