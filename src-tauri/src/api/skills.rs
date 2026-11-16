use crate::{app_state::AppState, db::workspaces, error::AppResult};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct SkillQuery {
    pub workspace_id: Option<String>,
    pub days: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GateDecision {
    pub approved: bool,
    #[serde(default)]
    pub note: Option<String>,
}

pub async fn evaluate(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SkillQuery>,
) -> AppResult<Json<Vec<crate::skill_eval::SkillUsage>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    Ok(Json(
        crate::skill_eval::evaluate(
            &state.db,
            &state.paths,
            std::path::Path::new(&workspace.root_path),
            query.days.unwrap_or(30),
        )
        .await?,
    ))
}

pub async fn gate_run(
    State(state): State<Arc<AppState>>,
    Path(skill_id): Path<String>,
    Query(query): Query<SkillQuery>,
    Json(thresholds): Json<crate::skill_gate::GateThresholds>,
) -> AppResult<Json<crate::skill_gate::SkillGate>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    Ok(Json(
        crate::skill_gate::run(
            &state.db,
            &state.paths,
            &workspace_id,
            std::path::Path::new(&workspace.root_path),
            &skill_id,
            thresholds,
        )
        .await?,
    ))
}

pub async fn gate_latest(
    State(state): State<Arc<AppState>>,
    Path(skill_id): Path<String>,
    Query(query): Query<SkillQuery>,
) -> AppResult<Json<Option<crate::skill_gate::SkillGate>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        crate::skill_gate::latest(&state.db, &workspace_id, &skill_id).await?,
    ))
}

pub async fn gate_decide(
    State(state): State<Arc<AppState>>,
    Path((skill_id, gate_id)): Path<(String, String)>,
    Query(query): Query<SkillQuery>,
    Json(body): Json<GateDecision>,
) -> AppResult<Json<crate::skill_gate::SkillGate>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let gate = crate::skill_gate::get(&state.db, &gate_id).await?;
    if gate.skill_id != skill_id {
        return Err(crate::error::AppError::NotFound(format!(
            "gate {gate_id} da skill {skill_id}"
        )));
    }
    Ok(Json(
        crate::skill_gate::decide(
            &state.db,
            &workspace_id,
            &gate_id,
            body.approved,
            body.note.as_deref(),
        )
        .await?,
    ))
}
