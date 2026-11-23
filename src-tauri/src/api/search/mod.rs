use crate::{
    app_state::AppState,
    config_sync::{agents, commands, paths::StudioPaths, skills},
    error::AppResult,
};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::Arc};

mod all;
mod ranking;
mod scope;

pub use all::{recent, search};

#[derive(Debug, Deserialize, Default)]
pub struct SearchQuery {
    #[serde(default)]
    pub q: String,
    pub workspace_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub snippet: String,
    pub updated_at: Option<String>,
    pub link: String,
}
