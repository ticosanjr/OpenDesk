//! Shell state for the Tauri desktop: either it owns the OpenDesk backend
//! (embedded mode) or it attaches to an already-running daemon discovered by
//! `daemon_probe` (remote mode).
//!
//! Remote mode never creates an `AppState`: the desktop must not open the
//! SQLite database, bind another API port or overwrite `api.json` while the
//! daemon owns those resources. The UI receives the remote endpoint through
//! `get_api_endpoint` and talks to it exactly like the embedded server.

use crate::{
    api_endpoint::base_url,
    app_state::{ApiEndpoint, AppState},
    paths::AppPaths,
};
use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Method};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{sync::Arc, time::Duration};

/// A thin authenticated client for a daemon owned by another process.
pub struct RemoteShell {
    pub endpoint: ApiEndpoint,
    pub paths: AppPaths,
    client: Client,
}

impl RemoteShell {
    pub fn new(endpoint: ApiEndpoint, paths: AppPaths) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            endpoint,
            paths,
            client,
        }
    }

    pub fn base_url(&self) -> String {
        base_url(&self.endpoint.host, self.endpoint.port)
    }

    pub async fn call(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base_url()))
            .bearer_auth(&self.endpoint.token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .context("falha ao chamar o daemon OpenDesk externo")?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            let message = serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|value| {
                    value
                        .get("message")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or(text);
            return Err(anyhow!("API {}: {}", status.as_u16(), message));
        }
        Ok(response.json().await.unwrap_or(Value::Null))
    }

    pub async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let value = self.call(Method::GET, path, None).await?;
        serde_json::from_value(value).context("resposta inesperada do daemon OpenDesk")
    }

    /// Liveness check used by the tray poller. It intentionally ignores the
    /// body: `/healthz` is unauthenticated and only proves the process is up.
    pub async fn health(&self) -> bool {
        let client = match Client::builder().timeout(Duration::from_secs(2)).build() {
            Ok(client) => client,
            Err(_) => return false,
        };
        client
            .get(format!("{}/healthz", self.base_url()))
            .send()
            .await
            .map(|response| response.status().is_success())
            .unwrap_or(false)
    }

    pub async fn workspace_roots(&self) -> Result<Vec<String>> {
        let workspaces: Vec<Value> = self.get_json("/api/workspaces").await?;
        Ok(workspaces
            .into_iter()
            .filter_map(|workspace| {
                workspace
                    .get("root_path")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect())
    }
}

/// The single managed state of the desktop shell. Commands receive this enum
/// instead of `AppState` so remote mode cannot accidentally touch the local
/// database.
pub enum ShellState {
    Embedded(Arc<AppState>),
    Remote(Arc<RemoteShell>),
}

impl ShellState {
    pub fn embedded(&self) -> Option<&Arc<AppState>> {
        match self {
            Self::Embedded(state) => Some(state),
            Self::Remote(_) => None,
        }
    }

    pub fn api_endpoint(&self) -> ApiEndpoint {
        match self {
            Self::Embedded(state) => state.api_endpoint(),
            Self::Remote(shell) => shell.endpoint.clone(),
        }
    }

    pub fn mode(&self) -> &'static str {
        if matches!(self, Self::Remote(_)) {
            "remote"
        } else {
            "embedded"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::paths_in;

    #[test]
    fn remote_shell_reports_its_mode_and_endpoint() {
        let dir = tempfile::tempdir().expect("temp");
        let paths = paths_in(dir.path());
        let endpoint = ApiEndpoint {
            host: "127.0.0.1".into(),
            port: 4242,
            token: "secret".into(),
        };
        let shell = ShellState::Remote(Arc::new(RemoteShell::new(endpoint.clone(), paths)));
        assert_eq!(shell.mode(), "remote");
        assert!(shell.embedded().is_none());
        assert_eq!(shell.api_endpoint().port, 4242);
        let ShellState::Remote(remote) = &shell else {
            panic!("expected remote shell");
        };
        assert_eq!(remote.base_url(), "http://127.0.0.1:4242");
    }

    #[tokio::test]
    async fn remote_calls_use_the_bearer_token() {
        use axum::{Router, http::StatusCode, middleware, response::IntoResponse, routing::get};

        async fn auth(
            request: axum::extract::Request,
            next: middleware::Next,
        ) -> axum::response::Response {
            let ok = request
                .headers()
                .get(axum::http::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                == Some("Bearer secret");
            if ok {
                next.run(request).await
            } else {
                StatusCode::UNAUTHORIZED.into_response()
            }
        }

        async fn queue() -> axum::Json<serde_json::Value> {
            axum::Json(serde_json::json!({ "paused": true, "running": 0, "waiting": 2 }))
        }

        let app = Router::new()
            .route("/api/queue/status", get(queue))
            .layer(middleware::from_fn(auth))
            .route("/healthz", get(|| async { axum::http::StatusCode::OK }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let dir = tempfile::tempdir().expect("temp");
        let paths = paths_in(dir.path());
        let shell = RemoteShell::new(
            ApiEndpoint {
                host: "127.0.0.1".into(),
                port,
                token: "secret".into(),
            },
            paths,
        );
        let status: Value = shell.get_json("/api/queue/status").await.expect("status");
        assert_eq!(status["waiting"], 2);
        assert!(shell.health().await);

        let wrong = RemoteShell::new(
            ApiEndpoint {
                host: "127.0.0.1".into(),
                port,
                token: "wrong".into(),
            },
            crate::test_support::paths_in(dir.path()),
        );
        assert!(wrong.get_json::<Value>("/api/queue/status").await.is_err());

        server.abort();
    }
}
