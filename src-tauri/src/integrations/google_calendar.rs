use crate::{
    app_state::AppState,
    db::calendar::{self, CalendarSource, ExternalEventInput},
    error::{AppError, AppResult},
    integrations::google_api::GoogleApi,
};
use chrono::{DateTime, Duration, NaiveDate, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize)]
pub struct SyncReport {
    pub source_id: String,
    pub source_name: String,
    pub imported: usize,
    pub cancelled: usize,
    pub full_sync: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

struct SyncOutcome {
    imported: usize,
    cancelled: usize,
    full_sync: bool,
    sync_token: Option<String>,
}

/// Convert one Google Calendar event into the local cache shape. All-day dates
/// are anchored at UTC midnight and treated as calendar dates, not instants.
pub fn parse_event(
    value: &Value,
    source_timezone: &str,
    source_color: &str,
) -> Option<ExternalEventInput> {
    let external_id = value.get("id")?.as_str()?.trim();
    if external_id.is_empty() {
        return None;
    }
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("confirmed")
        .to_uppercase();
    let start = value.get("start")?;
    let end = value.get("end")?;
    let all_day = start.get("date").and_then(Value::as_str).is_some();
    let timezone = start
        .get("timeZone")
        .and_then(Value::as_str)
        .or_else(|| value.get("timeZone").and_then(Value::as_str))
        .unwrap_or(source_timezone)
        .to_string();

    let (start_at, end_at) = if all_day {
        let start_date = start.get("date")?.as_str()?;
        let start_day = NaiveDate::parse_from_str(start_date, "%Y-%m-%d").ok()?;
        let end_day = end
            .get("date")
            .and_then(Value::as_str)
            .and_then(|value| NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
            .unwrap_or_else(|| start_day + Duration::days(1));
        let end_day = if end_day <= start_day {
            start_day
        } else {
            end_day - Duration::days(1)
        };
        (
            format!("{}T00:00:00Z", start_day.format("%Y-%m-%d")),
            format!("{}T23:59:59Z", end_day.format("%Y-%m-%d")),
        )
    } else {
        let start = DateTime::parse_from_rfc3339(start.get("dateTime")?.as_str()?)
            .ok()?
            .with_timezone(&Utc);
        let mut end = DateTime::parse_from_rfc3339(end.get("dateTime")?.as_str()?)
            .ok()?
            .with_timezone(&Utc);
        if end <= start {
            end = start + Duration::minutes(1);
        }
        (
            start.to_rfc3339_opts(SecondsFormat::Secs, true),
            end.to_rfc3339_opts(SecondsFormat::Secs, true),
        )
    };

    Some(ExternalEventInput {
        external_id: external_id.to_string(),
        title: value
            .get("summary")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("(sem título)")
            .to_string(),
        description: value
            .get("description")
            .and_then(Value::as_str)
            .map(str::to_string),
        location: value
            .get("location")
            .and_then(Value::as_str)
            .map(str::to_string),
        start_at,
        end_at,
        all_day,
        timezone,
        status,
        color_hex: Some(source_color.to_string()),
    })
}

async fn sync_once(
    api: &GoogleApi,
    pool: &sqlx::SqlitePool,
    source: &CalendarSource,
    sync_token: Option<&str>,
) -> AppResult<SyncOutcome> {
    let external_id = source
        .external_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("calendário Google sem external_id".into()))?;
    let full_sync = sync_token.is_none();
    let now = Utc::now();
    let time_min = (now - Duration::days(30)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let time_max = (now + Duration::days(180)).to_rfc3339_opts(SecondsFormat::Secs, true);
    let mut page_token: Option<String> = None;
    let mut next_sync_token = None;
    let mut imported = 0usize;
    let mut cancelled = 0usize;

    for _ in 0..40 {
        let response = api
            .calendar_events_page(
                &source.workspace_id,
                external_id,
                if full_sync { Some(&time_min) } else { None },
                if full_sync { Some(&time_max) } else { None },
                sync_token,
                page_token.as_deref(),
            )
            .await
            .map_err(|error| AppError::Internal(format!("{error:#}")))?;

        if let Some(items) = response.get("items").and_then(Value::as_array) {
            for item in items {
                let status = item
                    .get("status")
                    .and_then(Value::as_str)
                    .unwrap_or("confirmed")
                    .to_uppercase();
                let event_id = item.get("id").and_then(Value::as_str).unwrap_or_default();
                if status == "CANCELLED" {
                    cancelled += 1;
                    if let Some(event) = parse_event(item, &source.timezone, &source.color_hex) {
                        calendar::upsert_external_event(
                            pool,
                            &source.workspace_id,
                            &source.id,
                            &event,
                        )
                        .await?;
                    } else if !event_id.is_empty() {
                        calendar::mark_external_event_cancelled(pool, &source.id, event_id).await?;
                    }
                    continue;
                }
                if let Some(event) = parse_event(item, &source.timezone, &source.color_hex) {
                    calendar::upsert_external_event(pool, &source.workspace_id, &source.id, &event)
                        .await?;
                    imported += 1;
                }
            }
        }

        next_sync_token = response
            .get("nextSyncToken")
            .and_then(Value::as_str)
            .map(str::to_string);
        page_token = response
            .get("nextPageToken")
            .and_then(Value::as_str)
            .map(str::to_string);
        if page_token.is_none() {
            break;
        }
    }

    Ok(SyncOutcome {
        imported,
        cancelled,
        full_sync,
        sync_token: next_sync_token,
    })
}

pub async fn sync_source(state: &AppState, source_id: &str) -> AppResult<SyncReport> {
    let source = calendar::get_source(&state.db, source_id).await?;
    if source.provider != "GOOGLE" {
        return Err(AppError::BadRequest(
            "somente calendários Google podem ser sincronizados".into(),
        ));
    }
    let api = GoogleApi::new(state.google.clone());
    let had_token = source.sync_token.is_some();
    let outcome = match sync_once(&api, &state.db, &source, source.sync_token.as_deref()).await {
        Ok(outcome) => outcome,
        Err(error) if had_token && format!("{error:#}").contains("410") => {
            // Sync tokens expire. Fall back to a full window once.
            calendar::set_sync_state(&state.db, &source.id, None, None).await?;
            sync_once(&api, &state.db, &source, None).await?
        }
        Err(error) => return Err(error),
    };
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    calendar::set_sync_state(
        &state.db,
        &source.id,
        outcome.sync_token.as_deref(),
        Some(&now),
    )
    .await?;
    Ok(SyncReport {
        source_id: source.id,
        source_name: source.name,
        imported: outcome.imported,
        cancelled: outcome.cancelled,
        full_sync: outcome.full_sync,
        error: None,
    })
}

pub async fn sync_all(state: &AppState) -> Vec<SyncReport> {
    let Ok(sources) = calendar::list_all_sources(&state.db).await else {
        return Vec::new();
    };
    let mut reports = Vec::new();
    for source in sources {
        match sync_source(state, &source.id).await {
            Ok(report) => reports.push(report),
            Err(error) => reports.push(SyncReport {
                source_id: source.id,
                source_name: source.name,
                imported: 0,
                cancelled: 0,
                full_sync: false,
                error: Some(format!("{error:#}")),
            }),
        }
    }
    reports
}

pub async fn sync_loop(state: Arc<AppState>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(300)).await;
        for report in sync_all(&state).await {
            if let Some(error) = report.error {
                tracing::warn!(
                    source = %report.source_id,
                    error = %error,
                    "Google Calendar sync failed"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_all_day_events_as_utc_dates() {
        let value = json!({
            "id": "evt-1",
            "summary": "Feriado",
            "status": "confirmed",
            "start": { "date": "2026-11-10" },
            "end": { "date": "2026-11-11" }
        });
        let event = parse_event(&value, "America/Sao_Paulo", "#ff0000").expect("event");
        assert_eq!(event.external_id, "evt-1");
        assert!(event.all_day);
        assert_eq!(event.start_at, "2026-11-10T00:00:00Z");
        assert_eq!(event.end_at, "2026-11-10T23:59:59Z");
        assert_eq!(event.timezone, "America/Sao_Paulo");
    }

    #[test]
    fn parses_timed_events_and_normalizes_ranges() {
        let value = json!({
            "id": "evt-2",
            "summary": "Reunião",
            "description": "Pauta",
            "location": "Sala 2",
            "status": "tentative",
            "start": {
                "dateTime": "2026-11-10T12:00:00-03:00",
                "timeZone": "America/Sao_Paulo"
            },
            "end": {
                "dateTime": "2026-11-10T12:00:00-03:00",
                "timeZone": "America/Sao_Paulo"
            }
        });
        let event = parse_event(&value, "UTC", "#00ff00").expect("event");
        assert!(!event.all_day);
        assert_eq!(event.start_at, "2026-11-10T15:00:00Z");
        assert_eq!(event.end_at, "2026-11-10T15:01:00Z");
        assert_eq!(event.status, "TENTATIVE");
        assert_eq!(event.location.as_deref(), Some("Sala 2"));
    }

    #[test]
    fn ignores_events_without_identity_or_times() {
        assert!(parse_event(&json!({ "summary": "x" }), "UTC", "#000000").is_none());
        assert!(
            parse_event(
                &json!({ "id": "x", "start": {}, "end": {} }),
                "UTC",
                "#000000"
            )
            .is_none()
        );
    }
}
