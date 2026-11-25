use crate::{
    api::bundles::{
        BundleImportBody, envelope, expect_array, expect_str, validate_envelope, validate_items,
    },
    app_state::AppState,
    config_bundle,
    error::{AppError, AppResult},
    personal_context::{self, PersonalContextGroup, PersonalContextInput},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct PersonalContextQuery {
    pub workspace_id: Option<String>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PersonalContextQuery>,
) -> AppResult<Json<Vec<PersonalContextGroup>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        personal_context::list(&state.db, &workspace_id).await?,
    ))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PersonalContextQuery>,
    Json(mut input): Json<PersonalContextInput>,
) -> AppResult<Json<PersonalContextGroup>> {
    if input.scope.trim().eq_ignore_ascii_case("workspace") && input.workspace_id.is_none() {
        input.workspace_id = Some(
            crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?,
        );
    }
    if !input.scope.trim().eq_ignore_ascii_case("workspace")
        && !input.scope.trim().eq_ignore_ascii_case("global")
    {
        return Err(AppError::BadRequest(
            "scope deve ser 'workspace' ou 'global'".into(),
        ));
    }
    Ok(Json(personal_context::create(&state.db, input).await?))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(mut input): Json<PersonalContextInput>,
) -> AppResult<Json<PersonalContextGroup>> {
    let current = personal_context::get(&state.db, &id).await?;
    if input.scope.trim().eq_ignore_ascii_case("workspace") && input.workspace_id.is_none() {
        input.workspace_id = current.workspace_id;
    }
    Ok(Json(personal_context::update(&state.db, &id, input).await?))
}

pub async fn delete(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    personal_context::delete(&state.db, &id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn export(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PersonalContextQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let groups = personal_context::list(&state.db, &workspace_id).await?;
    let (global, local): (Vec<_>, Vec<_>) = groups
        .into_iter()
        .partition(|group| group.scope == "global");
    Ok(Json(envelope(
        "personal_context",
        Some(&workspace_id),
        serde_json::json!({ "global": global, "workspace": local }),
    )))
}

pub async fn import(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BundleImportBody>,
) -> AppResult<Json<serde_json::Value>> {
    let items = validate_envelope(&body.bundle, "personal_context")?;
    validate_items(&items)?;
    let groups = expect_array(&items, "global")?
        .iter()
        .chain(expect_array(&items, "workspace")?)
        .collect::<Vec<_>>();
    if groups.len() > crate::api::bundles::MAX_ITEMS {
        return Err(AppError::BadRequest(
            "bundle de contexto pessoal excede o limite de itens".into(),
        ));
    }
    for group in &groups {
        expect_str(group, "instructions")?;
        if let Some(scope) = group.get("scope").and_then(serde_json::Value::as_str)
            && !matches!(scope, "global" | "workspace")
        {
            return Err(AppError::BadRequest(format!("scope inválido: {scope}")));
        }
        if let Some(enabled) = group.get("is_enabled")
            && !enabled.is_boolean()
        {
            return Err(AppError::BadRequest("is_enabled deve ser booleano".into()));
        }
    }
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    let mode = body.mode.into();
    config_bundle::import_personal_context(&state.db, &workspace_id, &items, mode).await?;
    Ok(Json(serde_json::json!({
        "ok": true,
        "kind": "personal_context",
        "imported": groups.len(),
    })))
}
