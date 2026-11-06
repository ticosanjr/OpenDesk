use crate::{
    app_state::AppState,
    db::schedules::{self, CreateScheduleInput, Schedule, UpdateScheduleInput},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
    queue_scheduler::cron_engine,
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

pub async fn create_schedule(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateScheduleInput>,
) -> AppResult<Json<Schedule>> {
    let next = cron_engine::next_occurrence(&input.cron_expression, &input.timezone, Utc::now())?;
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
    Json(input): Json<UpdateScheduleInput>,
) -> AppResult<Json<Schedule>> {
    let schedule = schedules::update(&state.db, &id, input).await?;
    let next =
        cron_engine::next_occurrence(&schedule.cron_expression, &schedule.timezone, Utc::now())?;
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
