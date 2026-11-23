//! Slack configuration and token storage.

use crate::error::{AppError, AppResult};
use opendesk_storage::{repos::settings, secrets::SecretStore};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

pub const SETTING: &str = "slack";
pub const SECRET_NAMESPACE: &str = "slack";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    pub enabled: bool,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub channel_allowlist: Vec<String>,
    #[serde(default)]
    pub user_allowlist: Vec<String>,
    #[serde(default = "default_agent")]
    pub agent: String,
    #[serde(default)]
    pub notes: String,
}

fn default_agent() -> String {
    "build".into()
}

impl Default for SlackConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            workspace_id: None,
            channel_allowlist: Vec::new(),
            user_allowlist: Vec::new(),
            agent: default_agent(),
            notes: String::new(),
        }
    }
}

pub async fn load(pool: &SqlitePool) -> AppResult<SlackConfig> {
    let value = settings::get(pool, SETTING).await?;
    Ok(value
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default())
}

pub async fn save(pool: &SqlitePool, config: &SlackConfig) -> AppResult<()> {
    settings::set(
        pool,
        SETTING,
        &serde_json::to_value(config)
            .map_err(|error| AppError::Internal(format!("slack config: {error}")))?,
    )
    .await?;
    Ok(())
}

pub fn app_token(secrets: &SecretStore) -> Option<String> {
    secrets
        .get(SECRET_NAMESPACE, "app_token")
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
}

pub fn bot_token(secrets: &SecretStore) -> Option<String> {
    secrets
        .get(SECRET_NAMESPACE, "bot_token")
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
}

pub fn set_tokens(secrets: &SecretStore, app_token: &str, bot_token: &str) -> AppResult<()> {
    if !app_token.trim().is_empty() {
        secrets
            .set(SECRET_NAMESPACE, "app_token", app_token.trim())
            .map_err(crate::error::AppError::from)?;
    }
    if !bot_token.trim().is_empty() {
        secrets
            .set(SECRET_NAMESPACE, "bot_token", bot_token.trim())
            .map_err(crate::error::AppError::from)?;
    }
    Ok(())
}

pub fn clear_tokens(secrets: &SecretStore) -> AppResult<()> {
    secrets
        .delete(SECRET_NAMESPACE, "app_token")
        .map_err(crate::error::AppError::from)?;
    secrets
        .delete(SECRET_NAMESPACE, "bot_token")
        .map_err(crate::error::AppError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn saves_config_and_tokens() {
        let temp = TempDir::new().expect("temp");
        let pool = opendesk_storage::init(&temp.path().join("db.sqlite"))
            .await
            .expect("db");
        let config = SlackConfig {
            enabled: true,
            user_allowlist: vec!["u1".into()],
            ..Default::default()
        };
        save(&pool, &config).await.expect("save");
        assert_eq!(load(&pool).await.expect("load").user_allowlist, vec!["u1"]);
        let secrets = SecretStore::with_dir(temp.path().join("secrets"));
        set_tokens(&secrets, "app", "bot").expect("tokens");
        assert!(app_token(&secrets).is_some());
        assert!(bot_token(&secrets).is_some());
        clear_tokens(&secrets).expect("clear");
        assert!(app_token(&secrets).is_none());
    }
}
