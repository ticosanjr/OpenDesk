use crate::{
    app_state::AppState,
    db::workspaces,
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Query, State},
};
use chrono::{DateTime, Duration, NaiveDate, NaiveTime, SecondsFormat, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

mod events;
mod sources;
mod sync;

#[path = "../calendar_agenda/mod.rs"]
mod agenda_sources;

pub use events::{create_event, delete_event, get_event, list_events, update_event};
pub use sources::{delete_source, google_calendars, import_google, list_sources, update_source};
pub use sync::{sync_calendars, sync_source};

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
    pub execution_status: Option<String>,
    pub run_id: Option<String>,
    pub missed_id: Option<String>,
    pub executed: bool,
    pub editable: bool,
}

pub(super) fn parse_bound(value: &str, field: &str) -> AppResult<DateTime<Utc>> {
    if let Ok(value) = DateTime::parse_from_rfc3339(value) {
        return Ok(value.with_timezone(&Utc));
    }
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Ok(date.and_time(NaiveTime::MIN).and_utc());
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
