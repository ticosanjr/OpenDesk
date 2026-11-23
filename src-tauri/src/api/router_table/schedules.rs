//! Route definitions for schedules, calendar and missed-run recovery.

use crate::api::*;
use crate::app_state::AppState;
use axum::Router;
use axum::routing::{get, post};
use std::sync::Arc;

pub(super) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/missed-runs", get(missed_runs::list_missed))
        .route("/api/missed-runs/run-all", post(missed_runs::run_all))
        .route("/api/missed-runs/ignore-all", post(missed_runs::ignore_all))
        .route("/api/missed-runs/{id}/run", post(missed_runs::run_missed))
        .route(
            "/api/missed-runs/{id}/ignore",
            post(missed_runs::ignore_missed),
        )
        .route(
            "/api/calendar/events",
            get(calendar::list_events).post(calendar::create_event),
        )
        .route(
            "/api/calendar/events/{id}",
            get(calendar::get_event)
                .patch(calendar::update_event)
                .delete(calendar::delete_event),
        )
        .route("/api/calendar/sources", get(calendar::list_sources))
        .route(
            "/api/calendar/sources/{id}",
            axum::routing::patch(calendar::update_source).delete(calendar::delete_source),
        )
        .route(
            "/api/calendar/sources/{id}/sync",
            post(calendar::sync_source),
        )
        .route(
            "/api/calendar/google/calendars",
            get(calendar::google_calendars),
        )
        .route("/api/calendar/google/import", post(calendar::import_google))
        .route("/api/calendar/sync", post(calendar::sync_calendars))
        .route("/api/calendar/agenda", get(calendar::agenda))
        .route(
            "/api/schedules/{id}/sinks",
            get(io_bus::schedule_sinks).post(io_bus::set_schedule_sinks),
        )
        .route(
            "/api/schedules",
            get(schedules::list_schedules).post(schedules::create_schedule),
        )
        .route("/api/schedules/tick", post(schedules::tick_scheduler))
        .route("/api/schedules/preview", post(schedules::preview_schedule))
        .route("/api/schedules/{id}/runs", get(schedules::schedule_runs))
        .route("/api/schedules/{id}/dry-run", post(schedules::dry_run))
        .route(
            "/api/schedules/{id}/notify-test",
            post(schedules::test_notification),
        )
        .route(
            "/api/schedules/{id}",
            get(schedules::get_schedule)
                .patch(schedules::update_schedule)
                .delete(schedules::delete_schedule),
        )
        .route("/api/schedules/{id}/run", post(schedules::run_schedule_now))
        .route(
            "/api/schedules/{id}/active",
            post(schedules::set_schedule_active),
        )
}
