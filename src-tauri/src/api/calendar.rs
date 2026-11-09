use crate::{
    app_state::AppState,
    db::{
        calendar::{self, CalendarEvent, CreateEventInput, UpdateEventInput},
        queue, runs, schedules, workspaces,
    },
    error::{AppError, AppResult},
    integrations::{google_api::GoogleApi, google_calendar},
    queue_scheduler::cron_engine,
    recipes::{cron::CronTrigger, parser, store},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::{DateTime, Duration, NaiveDate, SecondsFormat, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[path = "calendar_agenda.rs"]
mod agenda_sources;

#[derive(Debug, Deserialize, Default)]
pub struct CalendarQuery {
    pub workspace_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ImportCalendar {
    pub external_id: String,
    pub name: String,
    #[serde(default)]
    pub color_hex: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ImportBody {
    #[serde(default)]
    pub calendars: Vec<ImportCalendar>,
}

#[derive(Debug, Deserialize, Default)]
pub struct SourceOptionsBody {
    pub is_visible: Option<bool>,
    pub sync_enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct AgendaResponse {
    pub from: String,
    pub to: String,
    pub items: Vec<AgendaItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgendaItem {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub start: String,
    pub end: Option<String>,
    pub all_day: bool,
    pub status: String,
    pub source: String,
    pub color: Option<String>,
    pub href: Option<String>,
    pub editable: bool,
}

pub(super) fn parse_bound(value: &str, field: &str) -> AppResult<DateTime<Utc>> {
    if let Ok(value) = DateTime::parse_from_rfc3339(value) {
        return Ok(value.with_timezone(&Utc));
    }
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Ok(date.and_hms_opt(0, 0, 0).expect("midnight").and_utc());
    }
    Err(AppError::BadRequest(format!(
        "{field} inválido; use RFC3339 ou YYYY-MM-DD"
    )))
}

fn range(query: &CalendarQuery) -> AppResult<(DateTime<Utc>, DateTime<Utc>)> {
    let now = Utc::now();
    let from = match query.from.as_deref() {
        Some(value) => parse_bound(value, "from")?,
        None => now - Duration::days(31),
    };
    let to = match query.to.as_deref() {
        Some(value) => parse_bound(value, "to")?,
        None => now + Duration::days(31),
    };
    if to <= from {
        return Err(AppError::BadRequest("to precisa ser depois de from".into()));
    }
    if to - from > Duration::days(400) {
        return Err(AppError::BadRequest(
            "intervalo máximo do calendário é 400 dias".into(),
        ));
    }
    Ok((from, to))
}

pub(super) fn rfc3339(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn validate_timezone(value: Option<&str>) -> AppResult<String> {
    let timezone = value.unwrap_or("UTC").trim();
    timezone
        .parse::<Tz>()
        .map_err(|_| AppError::BadRequest(format!("timezone inválido '{timezone}'")))?;
    Ok(timezone.to_string())
}

fn validate_status(value: Option<&str>) -> AppResult<String> {
    let status = value.unwrap_or("CONFIRMED").trim().to_uppercase();
    if matches!(status.as_str(), "CONFIRMED" | "TENTATIVE" | "CANCELLED") {
        Ok(status)
    } else {
        Err(AppError::BadRequest(format!(
            "status de evento inválido '{status}'"
        )))
    }
}

fn validate_color(value: Option<&str>) -> AppResult<Option<String>> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    if value.len() == 7
        && value.starts_with('#')
        && value[1..].chars().all(|c| c.is_ascii_hexdigit())
    {
        Ok(Some(value.to_lowercase()))
    } else {
        Err(AppError::BadRequest(format!(
            "cor inválida '{value}'; esperado #rrggbb"
        )))
    }
}

fn event_times(start: &str, end: &str) -> AppResult<(DateTime<Utc>, DateTime<Utc>)> {
    let start = parse_bound(start, "start_at")?;
    let end = parse_bound(end, "end_at")?;
    if end <= start {
        return Err(AppError::BadRequest(
            "end_at precisa ser depois de start_at".into(),
        ));
    }
    Ok((start, end))
}

fn google_dates(all_day: bool, start: DateTime<Utc>, end: DateTime<Utc>) -> (String, String) {
    if all_day {
        (
            start.format("%Y-%m-%d").to_string(),
            (end + Duration::days(1)).format("%Y-%m-%d").to_string(),
        )
    } else {
        (rfc3339(start), rfc3339(end))
    }
}

pub(super) fn status_for(start: DateTime<Utc>, end: DateTime<Utc>, now: DateTime<Utc>) -> String {
    if end < now {
        "PAST".into()
    } else if start <= now {
        "ACTIVE".into()
    } else {
        "FUTURE".into()
    }
}

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

pub async fn sync_calendars(
    State(state): State<Arc<AppState>>,
) -> AppResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "reports": google_calendar::sync_all(&state).await,
    })))
}

pub async fn sync_source(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<google_calendar::SyncReport>> {
    Ok(Json(google_calendar::sync_source(&state, &id).await?))
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

pub async fn agenda(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CalendarQuery>,
) -> AppResult<Json<AgendaResponse>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let (from, to) = range(&query)?;
    let now = Utc::now();
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    let workspace_color = Some(workspace.color_hex.clone());

    let mut items =
        agenda_sources::collect(&state, &workspace_id, from, to, now, workspace_color).await?;

    items.sort_by(|left, right| left.start.cmp(&right.start));
    items.truncate(1500);
    Ok(Json(AgendaResponse {
        from: rfc3339(from),
        to: rfc3339(to),
        items,
    }))
}
