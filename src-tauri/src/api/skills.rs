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

pub async fn list_cases(
    State(state): State<Arc<AppState>>,
    Path(skill_id): Path<String>,
    Query(query): Query<SkillQuery>,
) -> AppResult<Json<Vec<crate::skill_cases::SkillEvalCase>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        crate::skill_cases::list(&state.db, &workspace_id, &skill_id).await?,
    ))
}

pub async fn create_case(
    State(state): State<Arc<AppState>>,
    Path(skill_id): Path<String>,
    Query(query): Query<SkillQuery>,
    Json(input): Json<crate::skill_cases::CreateSkillCase>,
) -> AppResult<Json<crate::skill_cases::SkillEvalCase>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        crate::skill_cases::create(&state.db, &workspace_id, &skill_id, input).await?,
    ))
}

pub async fn delete_case(
    State(state): State<Arc<AppState>>,
    Path((skill_id, case_id)): Path<(String, String)>,
    Query(query): Query<SkillQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let deleted = crate::skill_cases::delete(&state.db, &workspace_id, &skill_id, &case_id).await?;
    Ok(Json(serde_json::json!({ "deleted": deleted })))
}

#[derive(Debug, Deserialize, Default)]
pub struct ShadowBody {
    #[serde(default)]
    pub case_ids: Option<Vec<String>>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub thresholds: crate::skill_shadow::ShadowThresholds,
}

#[derive(Debug, Deserialize)]
pub struct ShadowDecision {
    pub approved: bool,
    #[serde(default)]
    pub note: Option<String>,
}

pub async fn shadow_run(
    State(state): State<Arc<AppState>>,
    Path(skill_id): Path<String>,
    Query(query): Query<SkillQuery>,
    Json(body): Json<ShadowBody>,
) -> AppResult<Json<crate::skill_shadow::ShadowRun>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    let executor = crate::skill_shadow_exec::ShellShadowExecutor {
        state: state.clone(),
        model: body.model,
    };
    Ok(Json(
        crate::skill_shadow::run(
            &state.db,
            &workspace_id,
            std::path::Path::new(&workspace.root_path),
            &skill_id,
            body.case_ids.as_deref(),
            body.thresholds,
            &executor,
        )
        .await?,
    ))
}

pub async fn shadow_list(
    State(state): State<Arc<AppState>>,
    Path(skill_id): Path<String>,
    Query(query): Query<SkillQuery>,
) -> AppResult<Json<Vec<crate::skill_shadow::ShadowRun>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        crate::skill_shadow::list(&state.db, &workspace_id, &skill_id).await?,
    ))
}

pub async fn shadow_get(
    State(state): State<Arc<AppState>>,
    Path((skill_id, run_id)): Path<(String, String)>,
) -> AppResult<Json<serde_json::Value>> {
    let run = crate::skill_shadow::get(&state.db, &run_id).await?;
    if run.skill_id != skill_id {
        return Err(crate::error::AppError::NotFound(format!(
            "shadow run {run_id} da skill {skill_id}"
        )));
    }
    let results = crate::skill_shadow::results(&state.db, &run_id).await?;
    Ok(Json(serde_json::json!({ "run": run, "results": results })))
}

pub async fn shadow_promote(
    State(state): State<Arc<AppState>>,
    Path((skill_id, run_id)): Path<(String, String)>,
    Query(query): Query<SkillQuery>,
    Json(body): Json<ShadowDecision>,
) -> AppResult<Json<crate::skill_shadow::ShadowRun>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let run = crate::skill_shadow::get(&state.db, &run_id).await?;
    if run.skill_id != skill_id {
        return Err(crate::error::AppError::NotFound(format!(
            "shadow run {run_id} da skill {skill_id}"
        )));
    }
    Ok(Json(
        crate::skill_shadow::promote(
            &state.db,
            &workspace_id,
            &run_id,
            body.approved,
            body.note.as_deref(),
        )
        .await?,
    ))
}

pub async fn rollback(
    State(state): State<Arc<AppState>>,
    Path(skill_id): Path<String>,
    Query(query): Query<SkillQuery>,
) -> AppResult<Json<crate::config_sync::skills::SkillFile>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    Ok(Json(
        crate::skill_shadow::rollback(
            std::path::Path::new(&workspace.root_path),
            &state.paths.backups_dir,
            &skill_id,
        )
        .await?,
    ))
}
