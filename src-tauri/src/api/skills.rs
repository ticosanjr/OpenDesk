use crate::{app_state::AppState, db::workspaces, error::AppResult};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct SkillQuery {
    pub workspace_id: Option<String>,
    pub days: Option<i64>,
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
