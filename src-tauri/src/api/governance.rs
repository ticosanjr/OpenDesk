use crate::{
    app_state::AppState,
    config_sync::{agents, paths::StudioPaths},
    db::{tasks, workspaces},
    error::{AppError, AppResult},
    governance::{self, AgentGoalInput, AgentProfileInput, DelegationInput},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

#[derive(Debug, Deserialize, Default)]
pub struct GovernanceQuery {
    pub workspace_id: Option<String>,
}

async fn workspace_id(state: &AppState, query: &GovernanceQuery) -> AppResult<String> {
    crate::api::tasks::resolve_workspace_id(state, query.workspace_id.as_deref()).await
}

pub async fn overview(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GovernanceQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id = workspace_id(&state, &query).await?;
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    let root = PathBuf::from(&workspace.root_path);

    let mut agents: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut merge = |list: Vec<agents::AgentFile>, scope: &str| {
        for agent in list {
            agents.insert(
                agent.id.clone(),
                json!({
                    "id": agent.id,
                    "name": agent.name,
                    "description": agent.description,
                    "mode": agent.mode,
                    "avatar": agent.avatar,
                    "color": agent.color,
                    "model": agent.model,
                    "disabled": agent.disabled,
                    "scope": scope,
                }),
            );
        }
    };
    merge(agents::list(&StudioPaths::new(None), "global")?, "global");
    merge(
        agents::list(&StudioPaths::new(Some(root)), "workspace")?,
        "workspace",
    );

    let profiles = governance::list_profiles(&state.db, &workspace_id).await?;
    let mut profile_rows = Vec::with_capacity(profiles.len());
    for profile in profiles {
        let spent = governance::monthly_spend(&state.db, &workspace_id, &profile.agent_id).await?;
        let limit = profile.monthly_budget_usd;
        profile_rows.push(json!({
            "profile": profile,
            "monthly_spend_usd": spent,
            "budget_remaining_usd": limit.map(|limit| (limit - spent).max(0.0)),
            "budget_percent": limit.filter(|limit| *limit > 0.0).map(|limit| (spent / limit * 100.0).min(100.0)),
        }));
    }

    Ok(Json(json!({
        "agents": agents.into_values().collect::<Vec<_>>(),
        "profiles": profile_rows,
        "goals": governance::list_goals(&state.db, &workspace_id).await?,
        "performance": governance::performance(&state.db, &workspace_id).await?,
        "delegations": governance::list_delegations(&state.db, &workspace_id).await?,
    })))
}

pub async fn put_agent(
    State(state): State<Arc<AppState>>,
    Path(agent_id): Path<String>,
    Query(query): Query<GovernanceQuery>,
    Json(input): Json<AgentProfileInput>,
) -> AppResult<Json<governance::AgentProfile>> {
    let workspace_id = workspace_id(&state, &query).await?;
    Ok(Json(
        governance::upsert_profile(&state.db, &workspace_id, &agent_id, input).await?,
    ))
}

pub async fn create_goal(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GovernanceQuery>,
    Json(input): Json<AgentGoalInput>,
) -> AppResult<Json<governance::AgentGoal>> {
    let workspace_id = workspace_id(&state, &query).await?;
    Ok(Json(
        governance::create_goal(&state.db, &workspace_id, input).await?,
    ))
}

pub async fn update_goal(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<GovernanceQuery>,
    Json(input): Json<AgentGoalInput>,
) -> AppResult<Json<governance::AgentGoal>> {
    let workspace_id = workspace_id(&state, &query).await?;
    Ok(Json(
        governance::update_goal(&state.db, &workspace_id, &id, input).await?,
    ))
}

pub async fn delete_goal(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<GovernanceQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id = workspace_id(&state, &query).await?;
    governance::delete_goal(&state.db, &workspace_id, &id).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn create_delegation(
    State(state): State<Arc<AppState>>,
    Query(query): Query<GovernanceQuery>,
    Json(input): Json<DelegationInput>,
) -> AppResult<Json<governance::AgentDelegation>> {
    let workspace_id = workspace_id(&state, &query).await?;
    Ok(Json(
        governance::create_delegation(&state.db, &workspace_id, input).await?,
    ))
}

#[derive(Debug, Deserialize, Default)]
pub struct ResolveDelegationBody {
    pub approved: bool,
    #[serde(default)]
    pub resolved_by: Option<String>,
}

pub async fn resolve_delegation(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<GovernanceQuery>,
    Json(body): Json<ResolveDelegationBody>,
) -> AppResult<Json<governance::AgentDelegation>> {
    let workspace_id = workspace_id(&state, &query).await?;
    let delegation = governance::get_delegation(&state.db, &workspace_id, &id).await?;
    if delegation.status != "PENDING" {
        return Err(AppError::Conflict("delegação já resolvida".into()));
    }
    if !body.approved {
        return Ok(Json(
            governance::resolve_delegation(
                &state.db,
                &workspace_id,
                &id,
                "REJECTED",
                None,
                body.resolved_by.as_deref(),
                None,
            )
            .await?,
        ));
    }

    governance::check_budget(&state.db, &workspace_id, &delegation.to_agent).await?;
    let task = tasks::create(
        &state.db,
        &workspace_id,
        tasks::CreateTaskInput {
            title: format!("[Delegação] {}", delegation.title),
            description: delegation.prompt.clone(),
            execution_mode: Some("BUILD_DIRECT".into()),
            priority: Some(2),
            assigned_agent: Some(delegation.to_agent.clone()),
            preferred_model: None,
            effort_level: None,
            care_level: Some("standard".into()),
            notify_email: None,
            export_to_gdocs: false,
            tags: vec![
                "governance:delegation".into(),
                format!("from:{}", delegation.from_agent),
            ],
            origin: Some("DELEGATION".into()),
            parent_task_id: None,
            dedupe: None,
        },
    )
    .await?;

    let error = if task.deduplicated {
        None
    } else {
        crate::api::tasks::dispatch_task_internal(
            &state,
            &task.id,
            crate::api::tasks::DispatchBody {
                policy: None,
                manual_agent: Some(delegation.to_agent.clone()),
                manual_model: None,
                manual_effort: None,
                execution_mode: None,
            },
            None,
        )
        .await
        .err()
        .map(|error| format!("{error:#}"))
    };
    if error.is_some() {
        state
            .events
            .publish(crate::events::OpenDeskWsEvent::TaskUpdated {
                task_id: task.id.clone(),
                workspace_id: workspace_id.clone(),
                stage: task.status.clone(),
                phase_status: task.phase_status.clone(),
            });
    }
    Ok(Json(
        governance::resolve_delegation(
            &state.db,
            &workspace_id,
            &id,
            "APPROVED",
            Some(&task.id),
            body.resolved_by.as_deref(),
            error.as_deref(),
        )
        .await?,
    ))
}

pub async fn heartbeat_now(
    State(state): State<Arc<AppState>>,
    Path(agent_id): Path<String>,
    Query(query): Query<GovernanceQuery>,
) -> AppResult<Json<tasks::Task>> {
    let workspace_id = workspace_id(&state, &query).await?;
    let profile = governance::get_profile(&state.db, &workspace_id, &agent_id)
        .await?
        .ok_or_else(|| {
            AppError::BadRequest(format!("agente @{agent_id} sem perfil de governança"))
        })?;
    let task = governance::run_heartbeat(&state, &workspace_id, &agent_id).await?;
    governance::mark_heartbeat(
        &state.db,
        &workspace_id,
        &agent_id,
        profile.heartbeat_minutes,
    )
    .await?;
    Ok(Json(task))
}
