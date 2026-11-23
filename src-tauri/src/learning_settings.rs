use crate::{
    db::settings,
    error::{AppError, AppResult},
};
use sqlx::SqlitePool;

use super::LearningSettings;

const SETTINGS_PREFIX: &str = "learning:";

fn settings_key(workspace_id: &str) -> String {
    format!("{SETTINGS_PREFIX}{workspace_id}")
}

pub async fn get_settings(pool: &SqlitePool, workspace_id: &str) -> AppResult<LearningSettings> {
    let raw = settings::get(pool, &settings_key(workspace_id)).await?;
    let Some(value) = raw else {
        return Ok(LearningSettings::default());
    };
    let mut parsed: LearningSettings = serde_json::from_value(value).unwrap_or_default();
    parsed.min_occurrences = parsed.min_occurrences.clamp(1, 20);
    Ok(parsed)
}

pub async fn set_settings(
    pool: &SqlitePool,
    workspace_id: &str,
    mut settings: LearningSettings,
) -> AppResult<LearningSettings> {
    settings.min_occurrences = settings.min_occurrences.clamp(1, 20);
    let value = serde_json::to_value(&settings)
        .map_err(|error| AppError::Internal(format!("invalid learning settings: {error}")))?;
    settings::set(pool, &settings_key(workspace_id), &value).await?;
    Ok(settings)
}
