use crate::{
    app_state::AppState,
    error::AppResult,
    self_test::{self, SelfTestMode, SelfTestReport},
};
use axum::{Json, extract::State};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct SelfTestBody {
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub mode: SelfTestMode,
}

pub async fn run(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SelfTestBody>,
) -> AppResult<Json<SelfTestReport>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    Ok(Json(
        self_test::run(&state, &workspace_id, body.mode).await?,
    ))
}
