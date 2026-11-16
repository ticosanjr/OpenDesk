use crate::{
    app_state::AppState,
    db::tickets::{self, Ticket, UpdateTicketInput},
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
pub struct TicketQuery {
    pub workspace_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TicketQuery>,
) -> AppResult<Json<Vec<Ticket>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        tickets::list(
            &state.db,
            &workspace_id,
            query.status.as_deref(),
            query.limit.unwrap_or(100),
        )
        .await?,
    ))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Ticket>> {
    Ok(Json(tickets::get(&state.db, &id).await?))
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<UpdateTicketInput>,
) -> AppResult<Json<Ticket>> {
    let ticket = tickets::update(&state.db, &id, input).await?;
    state.events.publish(OpenDeskWsEvent::TicketUpdated {
        ticket_id: ticket.id.clone(),
        workspace_id: ticket.workspace_id.clone(),
        source: ticket.source.clone(),
        status: ticket.status.clone(),
    });
    Ok(Json(ticket))
}
