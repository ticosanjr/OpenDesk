use super::{
    CalendarQuery, event_times, google_dates, range, rfc3339, validate_color, validate_status,
    validate_timezone,
};
use crate::{
    app_state::AppState,
    db::calendar::{self, CalendarEvent, CreateEventInput, UpdateEventInput},
    error::{AppError, AppResult},
    integrations::{google_api::GoogleApi, google_calendar},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use std::sync::Arc;

pub async fn list_events(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CalendarQuery>,
) -> AppResult<Json<Vec<CalendarEvent>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let (from, to) = range(&query)?;
    calendar::ensure_local_source(&state.db, &workspace_id).await?;
    Ok(Json(
        calendar::list_events_between(&state.db, &workspace_id, &rfc3339(from), &rfc3339(to), 1000)
            .await?,
    ))
}

pub async fn create_event(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CalendarQuery>,
    Json(input): Json<CreateEventInput>,
) -> AppResult<Json<CalendarEvent>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    if input.title.trim().is_empty() {
        return Err(AppError::BadRequest(
            "título do evento é obrigatório".into(),
        ));
    }
    let (start, end) = event_times(&input.start_at, &input.end_at)?;
    let timezone = validate_timezone(input.timezone.as_deref())?;
    let status = validate_status(input.status.as_deref())?;
    let color = validate_color(input.color_hex.as_deref())?;
    let source = match input.source_id.as_deref() {
        Some(id) => calendar::get_source(&state.db, id).await?,
        None => calendar::ensure_local_source(&state.db, &workspace_id).await?,
    };
    if source.workspace_id != workspace_id {
        return Err(AppError::BadRequest(
            "calendário pertence a outro workspace".into(),
        ));
    }

    if source.provider == "GOOGLE" {
        let external_id = source
            .external_id
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("calendário Google sem external_id".into()))?;
        let api = GoogleApi::new(state.google.clone());
        let (google_start, google_end) = google_dates(input.all_day, start, end);
        let value = api
            .calendar_create_in(
                &workspace_id,
                external_id,
                input.title.trim(),
                input.description.as_deref(),
                input.location.as_deref(),
                &google_start,
                &google_end,
                Some(&timezone),
                input.all_day,
            )
            .await
            .map_err(|error| {
                AppError::Internal(format!("falha ao criar evento no Google: {error:#}"))
            })?;
        let parsed = google_calendar::parse_event(&value, &source.timezone, &source.color_hex)
            .ok_or_else(|| AppError::Internal("Google não retornou um evento válido".into()))?;
        return Ok(Json(
            calendar::upsert_external_event(&state.db, &workspace_id, &source.id, &parsed).await?,
        ));
    }

    let event = calendar::create_event(
        &state.db,
        &workspace_id,
        &source.id,
        CreateEventInput {
            color_hex: color,
            ..input
        },
        &rfc3339(start),
        &rfc3339(end),
        &timezone,
        &status,
    )
    .await?;
    Ok(Json(event))
}

pub async fn get_event(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<CalendarEvent>> {
    Ok(Json(calendar::get_event(&state.db, &id).await?))
}

pub async fn update_event(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<UpdateEventInput>,
) -> AppResult<Json<CalendarEvent>> {
    let current = calendar::get_event(&state.db, &id).await?;
    let start_raw = input.start_at.as_deref().unwrap_or(&current.start_at);
    let end_raw = input.end_at.as_deref().unwrap_or(&current.end_at);
    let (start, end) = event_times(start_raw, end_raw)?;
    let timezone = validate_timezone(
        input
            .timezone
            .as_deref()
            .or(Some(current.timezone.as_str())),
    )?;
    let status = validate_status(input.status.as_deref().or(Some(current.status.as_str())))?;
    let color = validate_color(input.color_hex.as_deref().or(current.color_hex.as_deref()))?;
    if let Some(title) = input.title.as_deref()
        && title.trim().is_empty()
    {
        return Err(AppError::BadRequest(
            "título do evento é obrigatório".into(),
        ));
    }
    let title = input.title.clone().unwrap_or(current.title.clone());
    let description = input.description.clone().or(current.description.clone());
    let location = input.location.clone().or(current.location.clone());
    let all_day = input.all_day.unwrap_or(current.all_day);

    if let Some(source_id) = current.source_id.as_deref() {
        let source = calendar::get_source(&state.db, source_id).await?;
        if source.provider == "GOOGLE" {
            let external_id = source
                .external_id
                .as_deref()
                .ok_or_else(|| AppError::BadRequest("calendário Google sem external_id".into()))?;
            let event_id = current
                .external_id
                .as_deref()
                .ok_or_else(|| AppError::BadRequest("evento Google sem external_id".into()))?;
            let api = GoogleApi::new(state.google.clone());
            let (google_start, google_end) = google_dates(all_day, start, end);
            let value = api
                .calendar_patch(
                    &current.workspace_id,
                    external_id,
                    event_id,
                    &title,
                    description.as_deref(),
                    location.as_deref(),
                    &google_start,
                    &google_end,
                    Some(&timezone),
                    all_day,
                )
                .await
                .map_err(|error| {
                    AppError::Internal(format!("falha ao atualizar evento no Google: {error:#}"))
                })?;
            let parsed = google_calendar::parse_event(&value, &source.timezone, &source.color_hex)
                .ok_or_else(|| AppError::Internal("Google não retornou um evento válido".into()))?;
            return Ok(Json(
                calendar::upsert_external_event(
                    &state.db,
                    &current.workspace_id,
                    &source.id,
                    &parsed,
                )
                .await?,
            ));
        }
    }

    Ok(Json(
        calendar::update_event(
            &state.db,
            &id,
            UpdateEventInput {
                title: Some(title),
                description,
                location,
                all_day: Some(all_day),
                color_hex: color,
                ..input
            },
            &rfc3339(start),
            &rfc3339(end),
            &timezone,
            &status,
        )
        .await?,
    ))
}

pub async fn delete_event(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let event = calendar::get_event(&state.db, &id).await?;
    if let Some(source_id) = event.source_id.as_deref() {
        let source = calendar::get_source(&state.db, source_id).await?;
        if source.provider == "GOOGLE"
            && let (Some(calendar_id), Some(external_id)) =
                (source.external_id.as_deref(), event.external_id.as_deref())
        {
            let api = GoogleApi::new(state.google.clone());
            api.calendar_delete(&event.workspace_id, calendar_id, external_id)
                .await
                .map_err(|error| {
                    AppError::Internal(format!("falha ao excluir evento no Google: {error:#}"))
                })?;
        }
    }
    calendar::delete_event(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
