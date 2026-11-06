use crate::{
    app_state::AppState,
    db::schedules::{self, CreateScheduleInput, Schedule, UpdateScheduleInput},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
    queue_scheduler::{cron_engine, recurrence},
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use chrono::Utc;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct ScheduleQuery {
    pub workspace_id: Option<String>,
}

pub async fn list_schedules(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ScheduleQuery>,
) -> AppResult<Json<Vec<Schedule>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(schedules::list(&state.db, &workspace_id).await?))
}

#[derive(Debug, Deserialize)]
pub struct PreviewBody {
    pub recurrence: recurrence::Recurrence,
    pub timezone: String,
}

pub async fn preview_schedule(Json(body): Json<PreviewBody>) -> AppResult<Json<serde_json::Value>> {
    let cron_expression = body.recurrence.to_cron()?;
    let next_runs = recurrence::next_runs(&cron_expression, &body.timezone, Utc::now(), 5)?;
    Ok(Json(serde_json::json!({
        "cron_expression": cron_expression,
        "human": body.recurrence.human(),
        "next_runs": next_runs,
    })))
}

pub async fn create_schedule(
    State(state): State<Arc<AppState>>,
    Json(mut input): Json<CreateScheduleInput>,
) -> AppResult<Json<Schedule>> {
    let parsed = input
        .recurrence_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<recurrence::Recurrence>(value).ok());
    let next = if let Some(recurrence) = &parsed {
        input.cron_expression = recurrence.to_cron()?;
        input.one_time = recurrence.frequency == "once";
        if input.one_time {
            recurrence.once_datetime(&input.timezone)?
        } else {
            cron_engine::next_occurrence(&input.cron_expression, &input.timezone, Utc::now())?
        }
    } else {
        cron_engine::next_occurrence(&input.cron_expression, &input.timezone, Utc::now())?
    };
    if input.one_time && next <= Utc::now() {
        return Err(AppError::BadRequest(
            "a rotina de uma vez precisa estar no futuro".into(),
        ));
    }
    let schedule = schedules::create(&state.db, input).await?;
    schedules::set_next_run(&state.db, &schedule.id, Some(&next.to_rfc3339())).await?;
    let schedule = schedules::get(&state.db, &schedule.id).await?;
    state.events.publish(OpenDeskWsEvent::ScheduleUpdated {
        schedule_id: schedule.id.clone(),
        workspace_id: schedule.workspace_id.clone(),
    });
    Ok(Json(schedule))
}

pub async fn get_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Schedule>> {
    Ok(Json(schedules::get(&state.db, &id).await?))
}

pub async fn update_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(mut input): Json<UpdateScheduleInput>,
) -> AppResult<Json<Schedule>> {
    let parsed = input
        .recurrence_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<recurrence::Recurrence>(value).ok());
    if let Some(recurrence) = &parsed {
        input.cron_expression = Some(recurrence.to_cron()?);
        input.one_time = Some(recurrence.frequency == "once");
    }
    let schedule = schedules::update(&state.db, &id, input).await?;
    let next = if schedule.one_time {
        recurrence::parse_recurrence(schedule.recurrence_json.as_deref())
            .map(|recurrence| recurrence.once_datetime(&schedule.timezone))
            .transpose()?
            .unwrap_or_else(|| {
                cron_engine::next_occurrence(
                    &schedule.cron_expression,
                    &schedule.timezone,
                    Utc::now(),
                )
                .unwrap_or_else(|_| Utc::now())
            })
    } else {
        cron_engine::next_occurrence(&schedule.cron_expression, &schedule.timezone, Utc::now())?
    };
    schedules::set_next_run(&state.db, &id, Some(&next.to_rfc3339())).await?;
    let schedule = schedules::get(&state.db, &id).await?;
    state.events.publish(OpenDeskWsEvent::ScheduleUpdated {
        schedule_id: schedule.id.clone(),
        workspace_id: schedule.workspace_id.clone(),
    });
    Ok(Json(schedule))
}

pub async fn delete_schedule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    schedules::delete(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn run_schedule_now(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    crate::api::engine::require_engine(&state).await?;
    let job = state.cron.run_now(&id).await?;
    Ok(Json(serde_json::json!({ "ok": true, "job": job })))
}

#[derive(Debug, Deserialize)]
pub struct ActiveBody {
    pub active: bool,
}

pub async fn set_schedule_active(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ActiveBody>,
) -> AppResult<Json<Schedule>> {
    let schedule = schedules::set_active(&state.db, &id, body.active).await?;
    if body.active {
        let next = cron_engine::next_occurrence(
            &schedule.cron_expression,
            &schedule.timezone,
            Utc::now(),
        )?;
        schedules::set_next_run(&state.db, &id, Some(&next.to_rfc3339())).await?;
    }
    let schedule = schedules::get(&state.db, &id).await?;
    state.events.publish(OpenDeskWsEvent::ScheduleUpdated {
        schedule_id: schedule.id.clone(),
        workspace_id: schedule.workspace_id.clone(),
    });
    Ok(Json(schedule))
}

#[derive(Debug, Deserialize)]
pub struct TickQuery {
    #[allow(dead_code)]
    pub force: Option<bool>,
}

pub async fn tick_scheduler(
    State(state): State<Arc<AppState>>,
    Query(_query): Query<TickQuery>,
) -> AppResult<Json<cron_engine::CronTickResult>> {
    let result = state.cron.tick_once().await?;
    Ok(Json(result))
}

#[allow(dead_code)]
fn _error(_: AppError) {}
