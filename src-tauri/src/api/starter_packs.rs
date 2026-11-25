use crate::{
    api::bundles::{BundleImportBody, envelope, validate_envelope},
    app_state::AppState,
    db::workspaces,
    error::{AppError, AppResult},
};
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
    #[serde(default)]
    pub skip_conflicts: bool,
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

pub async fn list_items(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<PackQuery>,
) -> AppResult<Json<Vec<crate::starter_packs::PackItem>>> {
    let workspace_root = match query.workspace_id.as_deref() {
        Some(workspace_id) => workspaces::get(&state.db, workspace_id)
            .await
            .ok()
            .map(|workspace| std::path::PathBuf::from(workspace.root_path)),
        None => None,
    };
    Ok(Json(crate::starter_packs::items(
        &id,
        workspace_root.as_deref(),
    )?))
}

#[derive(Debug, Deserialize)]
pub struct InstallItemBody {
    pub workspace_id: Option<String>,
    pub kind: String,
    pub id: String,
    #[serde(default)]
    pub overwrite: bool,
}

pub async fn install_item(
    State(state): State<Arc<AppState>>,
    Path(pack_id): Path<String>,
    Json(body): Json<InstallItemBody>,
) -> AppResult<Json<crate::starter_packs::ItemInstallResult>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    Ok(Json(
        crate::starter_packs::install_item(
            &state,
            &pack_id,
            &workspace_id,
            &body.kind,
            &body.id,
            body.overwrite,
        )
        .await?,
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
        crate::starter_packs::install(
            &state,
            &id,
            &workspace_id,
            &body.overwrite,
            body.skip_conflicts,
        )
        .await?,
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

pub async fn export_pack(Path(id): Path<String>) -> AppResult<Json<serde_json::Value>> {
    let transfer = crate::starter_packs::export_pack(&id)?;
    Ok(Json(envelope(
        "starter_pack",
        None,
        serde_json::to_value(transfer)?,
    )))
}

pub async fn import_pack(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BundleImportBody>,
) -> AppResult<Json<serde_json::Value>> {
    let items = validate_envelope(&body.bundle, "starter_pack")?;
    let transfer: crate::starter_packs::PackTransfer = serde_json::from_value(items)
        .map_err(|error| AppError::BadRequest(format!("pacote inválido: {error}")))?;
    crate::starter_packs::validate_pack(&transfer)?;
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    let result = crate::starter_packs::import_pack(&state, &workspace_id, transfer).await?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "kind": "starter_pack",
        "imported": result,
    })))
}
