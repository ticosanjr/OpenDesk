mod cache;
mod callback;
mod flow;
#[cfg(test)]
mod tests;

use crate::{
    db::{integrations, settings},
    error::{AppError, AppResult},
    events::EventBus,
    secrets::SecretStore,
};
#[cfg(test)]
use flow::random_urlsafe;
use serde::Serialize;
use sqlx::SqlitePool;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration as StdDuration,
};

pub const GOOGLE_SCOPES: &str = "openid email profile \
https://www.googleapis.com/auth/drive.file \
https://www.googleapis.com/auth/documents \
https://www.googleapis.com/auth/calendar.events \
https://www.googleapis.com/auth/calendar.readonly \
https://www.googleapis.com/auth/gmail.send \
https://www.googleapis.com/auth/gmail.readonly \
https://www.googleapis.com/auth/tasks";

pub(super) const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub(super) const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub(super) const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
pub(super) const USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";

#[derive(Debug, Clone)]
pub struct GoogleClientConfig {
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GoogleStatus {
    pub connected: bool,
    pub email: Option<String>,
    pub scopes: Option<String>,
    pub callback_url: String,
    pub client_id_set: bool,
    pub client_secret_set: bool,
}

pub struct GoogleOAuth {
    pub(super) pool: SqlitePool,
    pub(super) secrets: Arc<SecretStore>,
    pub(super) events: EventBus,
    pub(super) http: reqwest::Client,
    pub(super) pending: Mutex<HashMap<String, flow::PendingFlow>>,
    pub(super) cache: Mutex<HashMap<String, cache::CachedToken>>,
    pub(super) refresh_lock: tokio::sync::Mutex<()>,
    pub(super) callback_port: u16,
    pub(super) callback_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl GoogleOAuth {
    pub fn new(pool: SqlitePool, secrets: Arc<SecretStore>, events: EventBus) -> Arc<Self> {
        let callback_port = std::env::var("OPENDESK_OAUTH_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(53682);
        Arc::new(Self {
            pool,
            secrets,
            events,
            http: reqwest::Client::builder()
                .timeout(StdDuration::from_secs(60))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            pending: Mutex::new(HashMap::new()),
            cache: Mutex::new(HashMap::new()),
            refresh_lock: tokio::sync::Mutex::new(()),
            callback_port,
            callback_task: Mutex::new(None),
        })
    }

    pub fn callback_url(&self) -> String {
        format!("http://127.0.0.1:{}/callback", self.callback_port)
    }

    pub async fn status(&self, workspace_id: &str) -> AppResult<GoogleStatus> {
        let integration = integrations::get(&self.pool, workspace_id).await?;
        let client_id = settings::get(&self.pool, "google_client_id").await?;
        let client_secret = self.secrets.get("google", "client_secret")?;
        let refresh = self.secrets.get("google", &refresh_key(workspace_id))?;

        Ok(GoogleStatus {
            connected: integration
                .as_ref()
                .and_then(|row| row.google_account_email.clone())
                .is_some()
                && refresh.is_some(),
            email: integration.and_then(|row| row.google_account_email),
            scopes: integrations::get(&self.pool, workspace_id)
                .await?
                .and_then(|row| row.google_scopes),
            callback_url: self.callback_url(),
            client_id_set: client_id
                .as_ref()
                .and_then(|value| value.as_str())
                .map(|value| !value.trim().is_empty())
                .unwrap_or(false),
            client_secret_set: client_secret.is_some(),
        })
    }

    pub async fn save_client_config(
        &self,
        client_id: &str,
        client_secret: Option<&str>,
    ) -> AppResult<()> {
        let client_id = client_id.trim();
        if client_id.is_empty() {
            return Err(AppError::BadRequest("google client_id is required".into()));
        }
        settings::set(
            &self.pool,
            "google_client_id",
            &serde_json::Value::String(client_id.to_string()),
        )
        .await?;
        if let Some(secret) = client_secret.filter(|secret| !secret.trim().is_empty()) {
            self.secrets.set("google", "client_secret", secret.trim())?;
        }
        Ok(())
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    pub(super) async fn load_client_config(&self) -> AppResult<GoogleClientConfig> {
        let client_id = settings::get(&self.pool, "google_client_id")
            .await?
            .and_then(|value| value.as_str().map(str::to_string))
            .or_else(|| std::env::var("GOOGLE_CLIENT_ID").ok())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                AppError::BadRequest(
                    "Google client_id is not configured. Create an OAuth client (Desktop app) in Google Cloud and save the credentials in Integrações.".into(),
                )
            })?;
        let client_secret = self
            .secrets
            .get("google", "client_secret")?
            .or_else(|| std::env::var("GOOGLE_CLIENT_SECRET").ok())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| AppError::BadRequest("Google client_secret is not configured".into()))?;

        Ok(GoogleClientConfig {
            client_id,
            client_secret,
        })
    }
}

pub(super) fn refresh_key(workspace_id: &str) -> String {
    format!("{workspace_id}:refresh")
}
