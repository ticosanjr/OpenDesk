use super::{TOKEN_URL, flow::TokenResponse, refresh_key};
use crate::error::{AppError, AppResult};
use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone)]
pub(crate) struct CachedToken {
    pub(super) access_token: String,
    pub(super) expires_at: DateTime<Utc>,
}

impl super::GoogleOAuth {
    pub async fn access_token(&self, workspace_id: &str) -> AppResult<String> {
        if let Some(token) = self.cached_token(workspace_id) {
            return Ok(token);
        }

        // Multiple MCP processes may request a refresh at the same time.
        // Serialize and re-check so only one network refresh happens.
        let _guard = self.refresh_lock.lock().await;
        if let Some(token) = self.cached_token(workspace_id) {
            return Ok(token);
        }
        let refresh_token = self
            .secrets
            .get("google", &refresh_key(workspace_id))?
            .ok_or_else(|| {
                AppError::BadRequest("Google Workspace is not connected for this workspace".into())
            })?;
        let config = self.load_client_config().await?;

        let response = self
            .http
            .post(TOKEN_URL)
            .form(&[
                ("client_id", config.client_id.as_str()),
                ("client_secret", config.client_secret.as_str()),
                ("refresh_token", refresh_token.as_str()),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await
            .map_err(|error| AppError::Daemon(format!("Google token refresh failed: {error}")))?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(AppError::BadRequest(format!(
                "Google refresh token rejected ({status}); reconnect the account. {body}"
            )));
        }
        let token: TokenResponse = serde_json::from_str(&body).map_err(|error| {
            AppError::Internal(format!("invalid Google token response: {error}"))
        })?;
        self.cache_token(workspace_id, token.access_token.clone(), token.expires_in);
        Ok(token.access_token)
    }

    pub(super) fn cached_token(&self, workspace_id: &str) -> Option<String> {
        let cache = self
            .cache
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        cache
            .get(workspace_id)
            .filter(|token| token.expires_at > Utc::now() + Duration::seconds(30))
            .map(|token| token.access_token.clone())
    }

    pub(super) fn cache_token(
        &self,
        workspace_id: &str,
        access_token: String,
        expires_in: Option<i64>,
    ) {
        let expires_at = Utc::now() + Duration::seconds(expires_in.unwrap_or(3300));
        self.cache
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .insert(
                workspace_id.to_string(),
                CachedToken {
                    access_token,
                    expires_at,
                },
            );
    }
}
