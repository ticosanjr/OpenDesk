use crate::{
    api::bundles::{BundleImportBody, envelope, validate_envelope, validate_items},
    app_state::AppState,
    dsh_router::prompt_templates::{self, PromptTemplateField, PromptTemplates},
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct TemplatesQuery {
    pub workspace_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TemplatesBody {
    #[serde(default)]
    pub workspace_id: Option<String>,
    pub templates: PromptTemplates,
}

#[derive(Debug, Serialize)]
pub struct TemplatesResponse {
    pub workspace_id: Option<String>,
    pub scope: String,
    pub templates: PromptTemplates,
    pub defaults: PromptTemplates,
    pub fields: &'static [PromptTemplateField],
    pub variables: &'static [prompt_templates::PromptVariable],
}

async fn resolve_workspace(state: &AppState, requested: Option<&str>) -> AppResult<Option<String>> {
    if let Some(id) = requested.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(Some(id.to_string()));
    }
    crate::db::settings::active_workspace_id(&state.db)
        .await
        .map_err(Into::into)
}

async fn response(state: &AppState, workspace_id: Option<String>) -> AppResult<TemplatesResponse> {
    let templates = PromptTemplates::load(&state.db, workspace_id.as_deref()).await?;
    Ok(TemplatesResponse {
        scope: if workspace_id.is_some() {
            "workspace".into()
        } else {
            "global".into()
        },
        workspace_id,
        templates,
        defaults: PromptTemplates::default(),
        fields: prompt_templates::FIELDS,
        variables: prompt_templates::VARIABLES,
    })
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TemplatesQuery>,
) -> AppResult<Json<TemplatesResponse>> {
    let workspace_id = resolve_workspace(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(response(&state, workspace_id).await?))
}

pub async fn put(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TemplatesBody>,
) -> AppResult<Json<TemplatesResponse>> {
    let workspace_id = resolve_workspace(&state, body.workspace_id.as_deref()).await?;
    PromptTemplates::save(&state.db, workspace_id.as_deref(), &body.templates).await?;
    Ok(Json(response(&state, workspace_id).await?))
}

pub async fn reset(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TemplatesQuery>,
) -> AppResult<Json<TemplatesResponse>> {
    let workspace_id = resolve_workspace(&state, body.workspace_id.as_deref()).await?;
    prompt_templates::reset(&state.db, workspace_id.as_deref()).await?;
    Ok(Json(response(&state, workspace_id).await?))
}

pub async fn export(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TemplatesQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id = resolve_workspace(&state, query.workspace_id.as_deref()).await?;
    let global = PromptTemplates::load(&state.db, None).await?;
    let workspace = match workspace_id.as_deref() {
        Some(id) => Some(PromptTemplates::load(&state.db, Some(id)).await?),
        None => None,
    };
    Ok(Json(envelope(
        "prompt_templates",
        workspace_id.as_deref(),
        serde_json::json!({ "global": global, "workspace": workspace }),
    )))
}

pub async fn import(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BundleImportBody>,
) -> AppResult<Json<serde_json::Value>> {
    let items = validate_envelope(&body.bundle, "prompt_templates")?;
    validate_items(&items)?;
    let known: BTreeSet<&str> = prompt_templates::FIELDS
        .iter()
        .map(|field| field.id)
        .collect();
    for scope in ["global", "workspace"] {
        let Some(value) = items.get(scope) else {
            continue;
        };
        if value.is_null() {
            continue;
        }
        let object = value
            .as_object()
            .ok_or_else(|| AppError::BadRequest(format!("items.{scope} deve ser um objeto")))?;
        let unknown = object
            .keys()
            .filter(|key| !known.contains(key.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if !unknown.is_empty() {
            return Err(AppError::BadRequest(format!(
                "chaves desconhecidas em {scope}: {}",
                unknown.join(", ")
            )));
        }
        for (key, entry) in object {
            if !entry.is_string() {
                return Err(AppError::BadRequest(format!(
                    "{scope}.{key} deve ser texto"
                )));
            }
        }
        let _: PromptTemplates = serde_json::from_value(value.clone())
            .map_err(|error| AppError::BadRequest(format!("{scope} inválido: {error}")))?;
    }
    let workspace_id = resolve_workspace(&state, body.workspace_id.as_deref()).await?;
    let mut imported = 0;
    if let Some(global) = items.get("global").filter(|value| !value.is_null()) {
        let parsed: PromptTemplates = serde_json::from_value(global.clone())?;
        PromptTemplates::save(&state.db, None, &parsed).await?;
        imported += 1;
    }
    if let Some(local) = items.get("workspace").filter(|value| !value.is_null()) {
        let workspace_id = workspace_id.as_deref().ok_or_else(|| {
            AppError::BadRequest("workspace_id é obrigatório para prompts de workspace".into())
        })?;
        let parsed: PromptTemplates = serde_json::from_value(local.clone())?;
        PromptTemplates::save(&state.db, Some(workspace_id), &parsed).await?;
        imported += 1;
    }
    if imported == 0 {
        return Err(AppError::BadRequest(
            "bundle não contém templates globais nem de workspace".into(),
        ));
    }
    Ok(Json(serde_json::json!({
        "ok": true,
        "kind": "prompt_templates",
        "imported": imported,
    })))
}
