use crate::{
    app_state::AppState,
    db::proposals::{self, ProposalInput},
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
pub struct ProposalQuery {
    pub workspace_id: Option<String>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ProposalQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(serde_json::json!({
        "proposals": proposals::pending(&state.db, &workspace_id).await?,
    })))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ProposalQuery>,
    Json(input): Json<ProposalInput>,
) -> AppResult<Json<proposals::ConfigProposal>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let proposal = proposals::create(&state.db, &workspace_id, input).await?;
    state.events.publish(OpenDeskWsEvent::ProposalCreated {
        proposal_id: proposal.id.clone(),
        workspace_id: proposal.workspace_id.clone(),
        kind: proposal.kind.clone(),
        title: proposal.title.clone(),
    });
    Ok(Json(proposal))
}

#[derive(Debug, Deserialize)]
pub struct ResolveBody {
    pub approved: bool,
    #[serde(default)]
    pub resolved_by: Option<String>,
}

pub async fn resolve(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ResolveBody>,
) -> AppResult<Json<proposals::ConfigProposal>> {
    Ok(Json(
        crate::proposals::resolve(&state, &id, body.approved, body.resolved_by.as_deref()).await?,
    ))
}
