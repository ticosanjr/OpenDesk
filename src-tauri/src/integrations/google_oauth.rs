use crate::{
    db::{integrations, settings},
    error::{AppError, AppResult},
    events::{EventBus, OpenDeskWsEvent},
    secrets::SecretStore,
};
use axum::{
    Router,
    extract::{Query, State},
    response::Html,
    routing::get,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration as StdDuration,
};
use tokio::net::TcpListener;

pub const GOOGLE_SCOPES: &str = "openid email profile \
https://www.googleapis.com/auth/drive.file \
https://www.googleapis.com/auth/documents \
https://www.googleapis.com/auth/calendar.events \
https://www.googleapis.com/auth/gmail.send \
https://www.googleapis.com/auth/tasks";

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const USERINFO_URL: &str = "https://www.googleapis.com/oauth2/v3/userinfo";

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

#[derive(Debug, Clone, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct UserInfo {
    email: Option<String>,
}

#[derive(Debug, Clone)]
struct PendingFlow {
    workspace_id: String,
    verifier: String,
    redirect_uri: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct CachedToken {
    access_token: String,
    expires_at: DateTime<Utc>,
}

pub struct GoogleOAuth {
    pool: SqlitePool,
    secrets: Arc<SecretStore>,
    events: EventBus,
    http: reqwest::Client,
    pending: Mutex<HashMap<String, PendingFlow>>,
    cache: Mutex<HashMap<String, CachedToken>>,
    callback_port: u16,
    callback_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
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
                .expect("Google HTTP client"),
            pending: Mutex::new(HashMap::new()),
            cache: Mutex::new(HashMap::new()),
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

    pub async fn start_flow(self: &Arc<Self>, workspace_id: &str) -> AppResult<String> {
        self.ensure_callback_server().await?;
        let config = self.load_client_config().await?;

        let state = uuid::Uuid::new_v4().simple().to_string();
        let verifier = random_urlsafe(32);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let redirect_uri = self.callback_url();

        {
            let mut pending = self.pending.lock().expect("oauth pending mutex");
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
        self.secrets.delete("google", &refresh_key(workspace_id))?;
        self.cache
            .lock()
            .expect("google token cache")
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

    pub async fn access_token(&self, workspace_id: &str) -> AppResult<String> {
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

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    async fn load_client_config(&self) -> AppResult<GoogleClientConfig> {
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

    async fn ensure_callback_server(self: &Arc<Self>) -> AppResult<()> {
        let already_running = {
            let task = self.callback_task.lock().expect("oauth callback task");
            task.as_ref()
                .map(|handle| !handle.is_finished())
                .unwrap_or(false)
        };
        if already_running {
            return Ok(());
        }

        let listener = TcpListener::bind(("127.0.0.1", self.callback_port))
            .await
            .map_err(|error| {
                AppError::Daemon(format!(
                    "could not bind Google OAuth callback on 127.0.0.1:{}: {error}",
                    self.callback_port
                ))
            })?;
        let app = Router::new()
            .route("/callback", get(callback))
            .with_state(Arc::clone(self));

        let handle = tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, app).await {
                tracing::warn!(error = %error, "Google OAuth callback server stopped");
            }
        });
        *self.callback_task.lock().expect("oauth callback task") = Some(handle);
        Ok(())
    }

    async fn exchange_code(&self, flow: &PendingFlow, code: &str) -> AppResult<()> {
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

    fn cached_token(&self, workspace_id: &str) -> Option<String> {
        let cache = self.cache.lock().expect("google token cache");
        cache
            .get(workspace_id)
            .filter(|token| token.expires_at > Utc::now() + Duration::seconds(30))
            .map(|token| token.access_token.clone())
    }

    fn cache_token(&self, workspace_id: &str, access_token: String, expires_in: Option<i64>) {
        let expires_at = Utc::now() + Duration::seconds(expires_in.unwrap_or(3300));
        self.cache.lock().expect("google token cache").insert(
            workspace_id.to_string(),
            CachedToken {
                access_token,
                expires_at,
            },
        );
    }
}

fn refresh_key(workspace_id: &str) -> String {
    format!("{workspace_id}:refresh")
}

fn random_urlsafe(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    rand::rngs::OsRng.fill_bytes(&mut buffer);
    URL_SAFE_NO_PAD.encode(buffer)
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn callback(
    State(oauth): State<Arc<GoogleOAuth>>,
    Query(query): Query<CallbackQuery>,
) -> Html<String> {
    if let Some(error) = query.error {
        return Html(page(
            "Conexão cancelada",
            &format!("O Google retornou: {error}"),
        ));
    }
    let Some(state) = query.state else {
        return Html(page("Requisição inválida", "state ausente."));
    };
    let Some(code) = query.code else {
        return Html(page("Requisição inválida", "code ausente."));
    };
    let flow = oauth
        .pending
        .lock()
        .expect("oauth pending mutex")
        .remove(&state);
    let Some(flow) = flow else {
        return Html(page(
            "Sessão expirada",
            "O state não foi encontrado. Inicie a conexão novamente.",
        ));
    };
    if Utc::now() - flow.created_at > Duration::minutes(15) {
        return Html(page(
            "Sessão expirada",
            "A conexão demorou mais de 15 minutos. Inicie novamente.",
        ));
    }

    match oauth.exchange_code(&flow, &code).await {
        Ok(()) => Html(page(
            "Google conectado",
            "Você pode fechar esta aba e voltar ao OpenDesk.",
        )),
        Err(error) => Html(page("Falha ao conectar", &format!("{error}"))),
    }
}

fn page(title: &str, message: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{title}</title>\
         <style>body{{background:#09090b;color:#e4e4e7;font-family:Inter,Arial,sans-serif;\
         display:flex;align-items:center;justify-content:center;height:100vh;margin:0}}\
         .card{{border:1px solid #27272a;border-radius:12px;padding:24px;max-width:520px}}\
         h1{{font-size:18px;margin:0 0 8px}}p{{color:#a1a1aa;font-size:13px;line-height:1.6}}</style>\
         </head><body><div class=\"card\"><h1>{title}</h1><p>{message}</p></div></body></html>",
        title = html_escape::encode_text(title),
        message = html_escape::encode_text(message)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_and_url_are_well_formed() {
        let verifier = random_urlsafe(32);
        assert!(verifier.len() >= 43);
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(challenge.len(), 43);
        assert!(!challenge.contains('='));
        assert!(GOOGLE_SCOPES.contains("drive.file"));
        assert!(GOOGLE_SCOPES.contains("gmail.send"));
        assert!(GOOGLE_SCOPES.contains("tasks"));
    }
}
