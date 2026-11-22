use crate::{
    adapters::{AdapterService, HarnessHealth, HarnessSpec, UpdateHarnessInput},
    app_state::AppState,
    error::AppResult,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, serde::Serialize)]
pub struct HarnessView {
    #[serde(flatten)]
    spec: HarnessSpec,
    runtime_capabilities: crate::runtime::RuntimeCapabilities,
    is_default: bool,
}

pub async fn list(State(state): State<Arc<AppState>>) -> AppResult<Json<Vec<HarnessView>>> {
    let default = crate::adapters::default_harness(&state.db).await?;
    let list = state
        .adapters
        .list()
        .into_iter()
        .map(|spec| HarnessView {
            runtime_capabilities: spec.capabilities(),
            is_default: spec.id == default,
            spec,
        })
        .collect();
    Ok(Json(list))
}

#[derive(Debug, Deserialize)]
pub struct UpdateBody {
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub health_args: Option<Vec<String>>,
    #[serde(default)]
    pub name: Option<String>,
}

pub async fn update(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateBody>,
) -> AppResult<Json<HarnessSpec>> {
    let spec = state
        .adapters
        .update(
            &state.db,
            &id,
            UpdateHarnessInput {
                command: body.command,
                args: body.args,
                health_args: body.health_args,
                name: body.name,
            },
        )
        .await?;
    Ok(Json(spec))
}

pub async fn health(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<HarnessHealth>> {
    let health = check(&state.adapters, &state, &id).await?;
    Ok(Json(health))
}

async fn check(
    adapters: &AdapterService,
    state: &Arc<AppState>,
    id: &str,
) -> AppResult<HarnessHealth> {
    if id == "opencode" {
        let client = state.daemon.client().await;
        let status = if client.is_some() { "READY" } else { "ERROR" };
        if let Some(client) = client {
            let info = client.info().await.ok();
            return Ok(HarnessHealth {
                id: id.into(),
                status: status.into(),
                version: info.map(|info| info.version),
                error: None,
            });
        }
        return Ok(HarnessHealth {
            id: id.into(),
            status: status.into(),
            version: None,
            error: Some("daemon OpenCode offline".into()),
        });
    }
    adapters.health(&state.db, id).await.map_err(Into::into)
}

#[derive(Debug, Deserialize)]
pub struct DefaultBody {
    pub harness: String,
}

pub async fn set_default(
    State(state): State<Arc<AppState>>,
    Json(body): Json<DefaultBody>,
) -> AppResult<Json<serde_json::Value>> {
    let value = crate::adapters::set_default_harness(&state.db, &body.harness).await?;
    Ok(Json(json!({ "default_harness": value })))
}

pub async fn runtime_info(
    State(state): State<Arc<AppState>>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    let list = state
        .adapters
        .list()
        .into_iter()
        .map(|spec| {
            json!({
                "id": spec.id,
                "name": spec.name,
                "kind": spec.kind,
                "status": spec.status,
                "capabilities": spec.capabilities(),
            })
        })
        .collect::<Vec<_>>();
    Ok((StatusCode::OK, Json(json!({ "adapters": list }))))
}
