//! Detects an already-running OpenDesk daemon before the desktop starts an
//! embedded one. The probe is intentionally conservative: only localhost and a
//! token from `api.json` are accepted.

use crate::{app_state::ApiEndpoint, error::AppResult, paths::AppPaths};
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

/// Returns the external endpoint only when `/healthz` answers within 1s.
pub async fn probe(paths: &AppPaths) -> AppResult<Option<ApiEndpoint>> {
    let Some(endpoint) = endpoint_from_lock(paths)? else {
        return Ok(None);
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .map_err(|error| crate::error::AppError::Internal(error.to_string()))?;
    let url = format!("http://{}:{}/healthz", endpoint.host, endpoint.port);
    match client.get(url).send().await {
        Ok(response) if response.status().is_success() => Ok(Some(endpoint)),
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_only_local_endpoints() {
        let dir = tempfile::tempdir().expect("temp");
        let paths = crate::test_support::paths_in(dir.path());
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
}
