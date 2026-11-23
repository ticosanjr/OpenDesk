use super::{CalendarQuery, ImportBody, SourceOptionsBody};
use crate::{
    app_state::AppState,
    db::calendar,
    error::{AppError, AppResult},
    integrations::{google_api::GoogleApi, google_calendar},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use std::sync::Arc;

pub async fn google_calendars(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CalendarQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let api = GoogleApi::new(state.google.clone());
    let value = api.calendar_list_calendars(&workspace_id).await.map_err(|error| {
        AppError::BadRequest(format!(
            "não foi possível listar calendários Google; reconecte a conta em Integrações → Google ({error:#})"
        ))
    })?;
    Ok(Json(value))
}

pub async fn import_google(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CalendarQuery>,
    Json(body): Json<ImportBody>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    if body.calendars.is_empty() {
        return Err(AppError::BadRequest(
            "selecione pelo menos um calendário Google".into(),
        ));
    }
    let mut sources = Vec::new();
    let mut reports = Vec::new();
    for item in body.calendars {
        let source = calendar::upsert_google_source(
            &state.db,
            &workspace_id,
            &item.external_id,
            &item.name,
            item.color_hex.as_deref(),
            item.timezone.as_deref().unwrap_or("UTC"),
        )
        .await?;
        match google_calendar::sync_source(&state, &source.id).await {
            Ok(report) => reports.push(report),
            Err(error) => reports.push(google_calendar::SyncReport {
                source_id: source.id.clone(),
                source_name: source.name.clone(),
                imported: 0,
                cancelled: 0,
                full_sync: false,
                error: Some(format!("{error:#}")),
            }),
        }
        sources.push(source);
    }
    Ok(Json(serde_json::json!({
        "sources": sources,
        "reports": reports,
    })))
}

pub async fn update_source(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<SourceOptionsBody>,
) -> AppResult<Json<calendar::CalendarSource>> {
    Ok(Json(
        calendar::update_source_options(&state.db, &id, body.is_visible, body.sync_enabled).await?,
    ))
}

pub async fn delete_source(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let source = calendar::get_source(&state.db, &id).await?;
    if source.provider != "GOOGLE" {
        return Err(AppError::BadRequest(
            "o calendário local não pode ser removido".into(),
        ));
    }
    calendar::delete_source(&state.db, &id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub async fn list_sources(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CalendarQuery>,
) -> AppResult<Json<Vec<calendar::CalendarSource>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    calendar::ensure_local_source(&state.db, &workspace_id).await?;
    Ok(Json(
        calendar::list_sources(&state.db, &workspace_id).await?,
    ))
}
