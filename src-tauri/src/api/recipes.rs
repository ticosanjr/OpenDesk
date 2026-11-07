use crate::{
    app_state::AppState,
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
) -> AppResult<Json<serde_json::Value>> {
    let index = store::get(&state.db, &id).await?;
    let yaml = store::yaml_for(&state.db, &id).await?;
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
    let workspace_id = match store::get(&state.db, &id).await {
        Ok(index) => index.workspace_id,
        Err(_) => {
            crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?
        }
    };
    let saved = store::save_yaml(&state.db, &workspace_id, &id, &body.yaml).await?;
    Ok(Json(saved))
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
    Json(body): Json<RunBody>,
) -> AppResult<Json<store::RecipeRun>> {
    let _index = store::get(&state.db, &id).await?;
    let yaml = store::yaml_for(&state.db, &id).await?;
    let recipe = parser::parse(&yaml)?;
    let trigger = if body.trigger_context.is_null() {
        serde_json::json!({ "manual": true })
    } else {
        body.trigger_context
    };
    Ok(Json(
        engine::start(state.clone(), recipe, trigger, body.dry_run).await?,
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
    Ok(Json(serde_json::json!({
        "recipes": store::list(&state.db, &workspace_id).await?,
    })))
}
