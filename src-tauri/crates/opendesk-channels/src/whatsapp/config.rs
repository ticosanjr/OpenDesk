use crate::error::{AppError, AppResult};
use opendesk_storage::{repos::settings, secrets::SecretStore};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

pub const WHATSAPP_SETTING: &str = "channels.whatsapp";
pub const WHATSAPP_SECRET_NAMESPACE: &str = "channel_whatsapp";
pub const WEBHOOK_NAMESPACE: &str = "channel_webhook";
pub const WEBHOOK_HEADER: &str = "x-opendesk-channel-token";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WhatsAppConfig {
    pub enabled: bool,
    /// `structure`, `cloud-api` or `baileys`.
    pub provider: String,
    pub phone_number: String,
    /// Meta Cloud API phone number id.
    pub phone_number_id: String,
    pub business_account_id: String,
    pub api_version: String,
    /// Optional workspace for inbound messages; defaults to the active one.
    pub workspace_id: Option<String>,
    pub notes: String,
}

impl Default for WhatsAppConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: "structure".into(),
            phone_number: String::new(),
            phone_number_id: String::new(),
            business_account_id: String::new(),
            api_version: "v21.0".into(),
            workspace_id: None,
            notes: String::new(),
        }
    }
}

/// Reads the persisted configuration; any malformed value falls back to the
/// defaults so a bad entry never blocks the integrations page.
pub async fn load_config(pool: &SqlitePool) -> AppResult<WhatsAppConfig> {
    Ok(settings::get(pool, WHATSAPP_SETTING)
        .await?
        .and_then(|value| serde_json::from_value::<WhatsAppConfig>(value).ok())
        .unwrap_or_default())
}

pub async fn save_config(pool: &SqlitePool, config: &WhatsAppConfig) -> AppResult<()> {
    settings::set(pool, WHATSAPP_SETTING, &serde_json::to_value(config)?).await?;
    Ok(())
}

/// Rejects providers the UI cannot render and the `cloud-api` combination that
/// would never connect.
pub fn validate_config(config: &WhatsAppConfig) -> AppResult<()> {
    if !["structure", "cloud-api", "baileys"].contains(&config.provider.as_str()) {
        return Err(AppError::BadRequest(
            "provider de WhatsApp inválido; use structure, cloud-api ou baileys".into(),
        ));
    }
    if config.provider == "cloud-api" && config.phone_number_id.trim().is_empty() {
        return Err(AppError::BadRequest(
            "cloud-api exige phone_number_id (Meta)".into(),
        ));
    }
    Ok(())
}

/// Stores, trims or removes the Cloud API credentials. `None` leaves the
/// existing secret untouched; an empty value deletes it.
pub fn apply_credentials(
    secrets: &SecretStore,
    access_token: Option<&str>,
    app_secret: Option<&str>,
) -> AppResult<()> {
    for (key, value) in [("access_token", access_token), ("app_secret", app_secret)] {
        match value.map(str::trim) {
            Some("") => secrets.delete(WHATSAPP_SECRET_NAMESPACE, key)?,
            Some(value) => secrets.set(WHATSAPP_SECRET_NAMESPACE, key, value)?,
            None => {}
        }
    }
    Ok(())
}

pub fn connected(secrets: &SecretStore, config: &WhatsAppConfig) -> AppResult<bool> {
    Ok(config.enabled
        && config.provider == "cloud-api"
        && !config.phone_number_id.trim().is_empty()
        && secrets
            .get(WHATSAPP_SECRET_NAMESPACE, "access_token")?
            .is_some())
}
