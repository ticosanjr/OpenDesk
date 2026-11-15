use crate::{
    app_state::AppState,
    dsh_router::prompt_templates::{self, PromptTemplateField, PromptTemplates},
    error::AppResult,
};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};
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
}

async fn resolve_workspace(state: &AppState, requested: Option<&str>) -> AppResult<Option<String>> {
    if let Some(id) = requested.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(Some(id.to_string()));
    }
    crate::db::settings::active_workspace_id(&state.db).await
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
