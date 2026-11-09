use crate::{app_state::AppState, audit, error::AppResult};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct AuditQuery {
    pub workspace_id: Option<String>,
    pub action: Option<String>,
    pub limit: Option<i64>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuditQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let entries = audit::list(
        &state.db,
        query.workspace_id.as_deref(),
        query.action.as_deref(),
        query.limit.unwrap_or(200),
    )
    .await?;
    let limits = state.rate_limiter.limits();
    Ok(Json(json!({
        "entries": entries,
        "rate_limits": {
            "read_per_minute": limits.read_per_minute,
            "write_per_minute": limits.write_per_minute,
            "sensitive_per_minute": limits.sensitive_per_minute,
        }
    })))
}
