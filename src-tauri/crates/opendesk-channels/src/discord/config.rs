//! Discord configuration and token storage.

use crate::error::{AppError, AppResult};
use opendesk_storage::{repos::settings, secrets::SecretStore};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

pub const SETTING: &str = "discord";
pub const SECRET_NAMESPACE: &str = "discord";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    pub enabled: bool,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub guild_allowlist: Vec<String>,
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

impl Default for DiscordConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            workspace_id: None,
            guild_allowlist: Vec::new(),
            channel_allowlist: Vec::new(),
            user_allowlist: Vec::new(),
            agent: default_agent(),
            notes: String::new(),
        }
    }
}

pub async fn load(pool: &SqlitePool) -> AppResult<DiscordConfig> {
    let value = settings::get(pool, SETTING).await?;
    Ok(value
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default())
}

pub async fn save(pool: &SqlitePool, config: &DiscordConfig) -> AppResult<()> {
    settings::set(
        pool,
        SETTING,
        &serde_json::to_value(config)
            .map_err(|error| AppError::Internal(format!("discord config: {error}")))?,
    )
    .await?;
    Ok(())
}

pub fn token(secrets: &SecretStore) -> Option<String> {
    secrets
        .get(SECRET_NAMESPACE, "bot_token")
        .ok()
        .flatten()
        .filter(|value| !value.trim().is_empty())
}

pub fn set_token(secrets: &SecretStore, token: &str) -> AppResult<()> {
    let token = token.trim();
    if token.is_empty() {
        return Err(AppError::BadRequest("token do Discord vazio".into()));
    }
    secrets
        .set(SECRET_NAMESPACE, "bot_token", token)
        .map_err(crate::error::AppError::from)
}

pub fn clear_token(secrets: &SecretStore) -> AppResult<()> {
    secrets
        .delete(SECRET_NAMESPACE, "bot_token")
        .map_err(crate::error::AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn saves_config_and_keeps_token_in_secret_store() {
        let temp = TempDir::new().expect("temp");
        let pool = opendesk_storage::init(&temp.path().join("db.sqlite"))
            .await
            .expect("db");
        let config = DiscordConfig {
            enabled: true,
            channel_allowlist: vec!["c1".into()],
            ..Default::default()
        };
        save(&pool, &config).await.expect("save");
        assert_eq!(
            load(&pool).await.expect("load").channel_allowlist,
            vec!["c1"]
        );
        let secrets = SecretStore::with_dir(temp.path().join("secrets"));
        set_token(&secrets, "token").expect("token");
        assert_eq!(token(&secrets).as_deref(), Some("token"));
        clear_token(&secrets).expect("clear");
        assert!(token(&secrets).is_none());
    }
}
