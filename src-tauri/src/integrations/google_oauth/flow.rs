use super::{AUTH_URL, GOOGLE_SCOPES, REVOKE_URL, TOKEN_URL, USERINFO_URL, refresh_key};
use crate::{
    db::integrations,
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize)]
pub(super) struct TokenResponse {
    pub(super) access_token: String,
    #[serde(default)]
    pub(super) refresh_token: Option<String>,
    #[serde(default)]
    pub(super) expires_in: Option<i64>,
    #[serde(default)]
    pub(super) scope: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct UserInfo {
    email: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingFlow {
    pub(super) workspace_id: String,
    pub(super) verifier: String,
    pub(super) redirect_uri: String,
    pub(super) created_at: DateTime<Utc>,
}

impl super::GoogleOAuth {
    pub async fn start_flow(self: &Arc<Self>, workspace_id: &str) -> AppResult<String> {
        self.ensure_callback_server().await?;
        let config = self.load_client_config().await?;

        let state = uuid::Uuid::new_v4().simple().to_string();
        let verifier = random_urlsafe(32);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let redirect_uri = self.callback_url();

        {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            pending.retain(|_, flow| Utc::now() - flow.created_at < Duration::minutes(15));
            pending.insert(
                state.clone(),
                PendingFlow {
                    workspace_id: workspace_id.to_string(),
                    verifier,
                    redirect_uri: redirect_uri.clone(),
                    created_at: Utc::now(),
                },
            );
        }

        let params = [
            ("client_id", config.client_id.as_str()),
            ("redirect_uri", redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", GOOGLE_SCOPES),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("include_granted_scopes", "true"),
            ("state", state.as_str()),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
        ];
        let query = params
            .iter()
            .map(|(key, value)| format!("{}={}", key, urlencoding::encode(value)))
            .collect::<Vec<_>>()
            .join("&");

        Ok(format!("{AUTH_URL}?{query}"))
    }

    pub async fn disconnect(&self, workspace_id: &str) -> AppResult<()> {
        // Revoke the grant at Google before forgetting the local refresh token.
        // A failure here must not block disconnecting locally, but the user
        // should still be able to see that the remote grant survived.
        if let Some(refresh_token) = self.secrets.get("google", &refresh_key(workspace_id))? {
            match self
                .http
                .post(REVOKE_URL)
                .form(&[("token", refresh_token.as_str())])
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {}
                Ok(response) => tracing::warn!(
                    status = %response.status(),
                    "Google token revocation was rejected; the local token was removed anyway"
                ),
                Err(error) => tracing::warn!(
                    error = %error,
                    "Google token revocation failed; the local token was removed anyway"
                ),
            }
        }
        self.secrets.delete("google", &refresh_key(workspace_id))?;
        self.cache
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .remove(workspace_id);
        integrations::clear_google(&self.pool, workspace_id).await?;
        self.events
            .publish(OpenDeskWsEvent::GoogleConnectionUpdated {
                workspace_id: workspace_id.to_string(),
                connected: false,
                email: None,
            });
        Ok(())
    }

    pub(super) async fn exchange_code(&self, flow: &PendingFlow, code: &str) -> AppResult<()> {
        let config = self.load_client_config().await?;
        let response = self
            .http
            .post(TOKEN_URL)
            .form(&[
                ("client_id", config.client_id.as_str()),
                ("client_secret", config.client_secret.as_str()),
                ("code", code),
                ("code_verifier", flow.verifier.as_str()),
                ("redirect_uri", flow.redirect_uri.as_str()),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await
            .map_err(|error| AppError::Daemon(format!("Google token exchange failed: {error}")))?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(AppError::BadRequest(format!(
                "Google token exchange rejected ({status}): {body}"
            )));
        }
        let token: TokenResponse = serde_json::from_str(&body).map_err(|error| {
            AppError::Internal(format!("invalid Google token response: {error}"))
        })?;
        let refresh_token = token.refresh_token.ok_or_else(|| {
            AppError::BadRequest(
                "Google did not return a refresh token; reconnect with prompt=consent".into(),
            )
        })?;

        self.secrets
            .set("google", &refresh_key(&flow.workspace_id), &refresh_token)?;
        self.cache_token(
            &flow.workspace_id,
            token.access_token.clone(),
            token.expires_in,
        );

        let email = match self
            .http
            .get(USERINFO_URL)
            .bearer_auth(&token.access_token)
            .send()
            .await
        {
            Ok(response) => response
                .json::<UserInfo>()
                .await
                .ok()
                .and_then(|info| info.email)
                .unwrap_or_else(|| "conta Google".to_string()),
            Err(_) => "conta Google".to_string(),
        };

        integrations::upsert_google(
            &self.pool,
            &flow.workspace_id,
            &email,
            token.scope.as_deref().unwrap_or(GOOGLE_SCOPES),
        )
        .await?;
        self.events
            .publish(OpenDeskWsEvent::GoogleConnectionUpdated {
                workspace_id: flow.workspace_id.clone(),
                connected: true,
                email: Some(email),
            });
        Ok(())
    }
}

pub(super) fn random_urlsafe(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    rand::rngs::OsRng.fill_bytes(&mut buffer);
    URL_SAFE_NO_PAD.encode(buffer)
}
