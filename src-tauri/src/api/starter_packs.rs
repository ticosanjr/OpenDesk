use crate::{app_state::AppState, error::AppResult};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct PackQuery {
    pub workspace_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct InstallBody {
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub overwrite: Vec<String>,
}

pub async fn list_packs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PackQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let (categories, packs) =
        crate::starter_packs::list(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(serde_json::json!({
        "categories": categories,
        "packs": packs,
    })))
}

pub async fn get_pack(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<PackQuery>,
) -> AppResult<Json<crate::starter_packs::PackDetail>> {
    Ok(Json(
        crate::starter_packs::detail(&state, &id, query.workspace_id.as_deref()).await?,
    ))
}

pub async fn preview_pack(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<PackQuery>,
) -> AppResult<Json<crate::starter_packs::InstallPlan>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    Ok(Json(
        crate::starter_packs::preview(&state, &id, &workspace_id).await?,
    ))
}

pub async fn install_pack(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<InstallBody>,
) -> AppResult<Json<crate::starter_packs::InstallResult>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    Ok(Json(
        crate::starter_packs::install(&state, &id, &workspace_id, &body.overwrite).await?,
    ))
}

pub async fn uninstall_pack(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<PackQuery>,
) -> AppResult<Json<crate::starter_packs::UninstallResult>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        crate::starter_packs::uninstall(&state, &id, &workspace_id).await?,
    ))
}

pub async fn list_templates() -> AppResult<Json<Vec<crate::starter_packs::WorkspaceTemplate>>> {
    Ok(Json(crate::starter_packs::templates()?))
}

#[derive(Debug, Deserialize)]
pub struct CreateTemplateBody {
    pub template_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub root_path: Option<String>,
    #[serde(default)]
    pub color: Option<String>,
}

pub async fn create_template(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateTemplateBody>,
) -> AppResult<Json<crate::starter_packs::TemplateResult>> {
    Ok(Json(
        crate::starter_packs::create_from_template(
            &state,
            &body.template_id,
            body.name.as_deref(),
            body.root_path.as_deref(),
            body.color.as_deref(),
        )
        .await?,
    ))
}
