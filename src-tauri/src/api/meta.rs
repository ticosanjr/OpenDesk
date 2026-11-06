use crate::{
    app_state::AppState,
    config_sync::paths::StudioPaths,
    db::workspaces,
    dsh_router::{self, MetaBuildRequest, MetaBuildResult},
    error::AppResult,
};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use std::{path::PathBuf, sync::Arc};

#[derive(Debug, Deserialize, Default)]
pub struct MetaQuery {
    pub workspace_id: Option<String>,
    pub scope: Option<String>,
}

pub async fn build(
    State(state): State<Arc<AppState>>,
    Query(query): Query<MetaQuery>,
    Json(request): Json<MetaBuildRequest>,
) -> AppResult<Json<MetaBuildResult>> {
    let scope = query.scope.unwrap_or_else(|| "workspace".to_string());
    let paths = if scope == "global" {
        StudioPaths::new(None)
    } else {
        let workspace_id =
            crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
        let workspace = workspaces::get(&state.db, &workspace_id).await?;
        StudioPaths::new(Some(PathBuf::from(workspace.root_path)))
    };

    Ok(Json(dsh_router::meta_build(
        &paths,
        &scope,
        request,
        &state.paths.backups_dir,
    )?))
}
