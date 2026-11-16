//! Detects an already-running OpenDesk daemon before the desktop starts an
//! embedded one. The probe is intentionally conservative: only localhost and a
//! token from `api.json` are accepted.

use crate::{api_endpoint::ApiEndpoint, api_endpoint::base_url, error::AppResult, paths::AppPaths};
use std::time::Duration;

pub fn endpoint_from_lock(paths: &AppPaths) -> AppResult<Option<ApiEndpoint>> {
    let raw = match std::fs::read_to_string(&paths.api_lock_path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let endpoint: ApiEndpoint = serde_json::from_str(&raw)
        .map_err(|error| crate::error::AppError::Internal(format!("api.json inválido: {error}")))?;
    if endpoint.token.trim().is_empty() || endpoint.port == 0 {
        return Ok(None);
    }
    if !matches!(endpoint.host.as_str(), "127.0.0.1" | "localhost" | "::1") {
        return Ok(None);
    }
    Ok(Some(endpoint))
}

/// Result of the startup probe. A corrupt or unreadable `api.json` must never
/// prevent the desktop from starting, so those cases degrade to `Embedded`.
#[derive(Debug, Clone)]
pub enum ProbeOutcome {
    Embedded,
    Remote(ApiEndpoint),
}

pub async fn probe_outcome(paths: &AppPaths) -> ProbeOutcome {
    let endpoint = match endpoint_from_lock(paths) {
        Ok(Some(endpoint)) => endpoint,
        Ok(None) => return ProbeOutcome::Embedded,
        Err(error) => {
            tracing::warn!(error = %error, "api.json inválido; iniciando backend embarcado");
            return ProbeOutcome::Embedded;
        }
    };
    if healthz(&endpoint).await {
        ProbeOutcome::Remote(endpoint)
    } else {
        ProbeOutcome::Embedded
    }
}

/// Returns the external endpoint only when `/healthz` answers within 1s.
async fn healthz(endpoint: &ApiEndpoint) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
    {
        Ok(client) => client,
        Err(_) => return false,
    };
    let url = format!("{}/healthz", base_url(&endpoint.host, endpoint.port));
    match client.get(url).send().await {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_local_endpoints() {
        let dir = tempfile::tempdir().expect("temp");
        let paths = crate::paths::test_paths(dir.path());
        assert!(endpoint_from_lock(&paths).expect("missing").is_none());
        std::fs::write(
            &paths.api_lock_path,
            r#"{"host":"127.0.0.1","port":1234,"token":"tok"}"#,
        )
        .expect("lock");
        let endpoint = endpoint_from_lock(&paths).expect("read").expect("endpoint");
        assert_eq!(endpoint.port, 1234);

        std::fs::write(
            &paths.api_lock_path,
            r#"{"host":"10.0.0.5","port":1234,"token":"tok"}"#,
        )
        .expect("lock");
        assert!(endpoint_from_lock(&paths).expect("remote").is_none());
    }

    #[tokio::test]
    async fn corrupt_lock_degrades_to_embedded() {
        let dir = tempfile::tempdir().expect("temp");
        let paths = crate::paths::test_paths(dir.path());
        std::fs::write(&paths.api_lock_path, "not json").expect("lock");
        assert!(matches!(
            probe_outcome(&paths).await,
            ProbeOutcome::Embedded
        ));
    }

    #[tokio::test]
    async fn healthy_daemon_is_attached() {
        let app = axum::Router::new().route(
            "/healthz",
            axum::routing::get(|| async { axum::http::StatusCode::OK }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let port = listener.local_addr().expect("addr").port();
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        let dir = tempfile::tempdir().expect("temp");
        let paths = crate::paths::test_paths(dir.path());
        std::fs::write(
            &paths.api_lock_path,
            format!(r#"{{"host":"127.0.0.1","port":{port},"token":"tok"}}"#),
        )
        .expect("lock");
        let outcome = probe_outcome(&paths).await;
        match outcome {
            ProbeOutcome::Remote(endpoint) => assert_eq!(endpoint.port, port),
            ProbeOutcome::Embedded => panic!("expected remote"),
        }
        server.abort();
    }
}
