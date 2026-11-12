use crate::{
    app_state::AppState,
    db::{missed_runs, schedules},
    error::AppResult,
    events::OpenDeskWsEvent,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct MissedQuery {
    pub workspace_id: Option<String>,
    pub status: Option<String>,
}

pub async fn list_missed(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MissedQuery>,
) -> AppResult<Json<Vec<missed_runs::MissedRun>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        missed_runs::list_for_workspace(&state.db, &workspace_id, query.status.as_deref()).await?,
    ))
}

pub async fn run_missed(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<missed_runs::MissedRun>> {
    let missed = missed_runs::get(&state.db, &id).await?;
    let result = state.cron.run_missed(&missed).await;
    let updated = match result {
        Ok(run_id) => {
            missed_runs::set_status(&state.db, &id, "QUEUED", run_id.as_deref(), None).await?
        }
        Err(error) => {
            missed_runs::set_status(&state.db, &id, "FAILED", None, Some(&format!("{error:#}")))
                .await?
        }
    };
    let schedule = schedules::get(&state.db, &missed.schedule_id).await?;
    state.events.publish(OpenDeskWsEvent::ScheduleUpdated {
        schedule_id: schedule.id,
        workspace_id: schedule.workspace_id,
    });
    Ok(Json(updated))
}

pub async fn ignore_missed(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<missed_runs::MissedRun>> {
    let updated = missed_runs::set_status(&state.db, &id, "IGNORED", None, None).await?;
    Ok(Json(updated))
}

pub async fn run_all(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MissedQuery>,
) -> AppResult<Json<Vec<missed_runs::MissedRun>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let pending = missed_runs::list_pending(&state.db, &workspace_id).await?;
    let mut updated = Vec::new();
    for missed in pending {
        let run_id = state.cron.run_missed(&missed).await.ok().flatten();
        updated.push(
            missed_runs::set_status(&state.db, &missed.id, "QUEUED", run_id.as_deref(), None)
                .await?,
        );
    }
    Ok(Json(updated))
}

pub async fn ignore_all(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MissedQuery>,
) -> AppResult<Json<Vec<missed_runs::MissedRun>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let pending = missed_runs::list_pending(&state.db, &workspace_id).await?;
    let mut updated = Vec::new();
    for missed in pending {
        updated.push(missed_runs::set_status(&state.db, &missed.id, "IGNORED", None, None).await?);
    }
    Ok(Json(updated))
}
