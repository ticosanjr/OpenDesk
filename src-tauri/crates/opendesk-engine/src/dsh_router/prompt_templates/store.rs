use super::PromptTemplates;
use crate::error::{AppError, AppResult};
use opendesk_storage::repos::settings;

use serde_json::Value;

const GLOBAL_KEY: &str = "prompt_templates";

pub async fn load(
    pool: &sqlx::SqlitePool,
    workspace_id: Option<&str>,
) -> AppResult<PromptTemplates> {
    if let Some(workspace_id) = workspace_id
        && let Some(value) = settings::get(pool, &workspace_key(workspace_id)).await?
    {
        return parse(value);
    }
    if let Some(value) = settings::get(pool, GLOBAL_KEY).await? {
        return parse(value);
    }
    Ok(PromptTemplates::default())
}

pub async fn save(
    pool: &sqlx::SqlitePool,
    workspace_id: Option<&str>,
    templates: &PromptTemplates,
) -> AppResult<()> {
    let value = serde_json::to_value(templates)
        .map_err(|error| AppError::Internal(format!("invalid prompt templates: {error}")))?;
    let key = workspace_id
        .map(workspace_key)
        .unwrap_or_else(|| GLOBAL_KEY.to_string());
    settings::set(pool, &key, &value).await.map_err(Into::into)
}

pub async fn reset(pool: &sqlx::SqlitePool, workspace_id: Option<&str>) -> AppResult<()> {
    match workspace_id {
        Some(id) => settings::delete(pool, &workspace_key(id))
            .await
            .map_err(Into::into),
        None => settings::delete(pool, GLOBAL_KEY).await.map_err(Into::into),
    }
}

fn workspace_key(workspace_id: &str) -> String {
    format!("{GLOBAL_KEY}:{workspace_id}")
}

fn parse(value: Value) -> AppResult<PromptTemplates> {
    serde_json::from_value(value)
        .map_err(|error| AppError::BadRequest(format!("prompt templates inválidos: {error}")))
}
