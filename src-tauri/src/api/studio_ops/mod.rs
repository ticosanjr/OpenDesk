use crate::{
    app_state::AppState,
    config_sync::{
        paths::StudioPaths,
        studio_item::{ItemSnapshot, StudioItem, StudioKind},
    },
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};

use super::studio::{StudioQuery, context, require_id};

mod catalog;
mod copy;
mod effective;
mod install;
mod runtime;

#[cfg(test)]
mod tests;

pub use catalog::catalog;
pub use copy::{copy, duplicate, list_backups, move_item, remove_item, restore_backup, toggle};
pub use effective::effective;
pub use install::*;
pub use runtime::runtime;

#[derive(Debug, Deserialize)]
pub struct TargetBody {
    #[serde(default)]
    pub target_scope: Option<String>,
    #[serde(default)]
    pub target_workspace_id: Option<String>,
    #[serde(default)]
    pub new_id: Option<String>,
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Deserialize)]
pub struct ToggleBody {
    pub enabled: bool,
}

struct Target {
    paths: StudioPaths,
    scope: String,
}

async fn target(state: &AppState, body: &TargetBody, source_scope: &str) -> AppResult<Target> {
    let query = StudioQuery {
        scope: Some(
            body.target_scope
                .clone()
                .unwrap_or_else(|| source_scope.to_string()),
        ),
        workspace_id: body.target_workspace_id.clone(),
        id: None,
        expected_sha256: None,
        location: None,
    };
    let (paths, scope, _) = context(state, &query).await?;
    Ok(Target { paths, scope })
}
