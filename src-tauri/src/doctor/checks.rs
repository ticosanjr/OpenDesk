use crate::opencode_client::rest_client::RestClient;
use serde_json::Value;
use std::path::{Path, PathBuf};

use super::ServiceProbe;

pub(super) async fn probe_daemon(lock_path: &Path) -> Option<ServiceProbe> {
    let bytes = std::fs::read(lock_path).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    let port = value.get("port").and_then(Value::as_u64).unwrap_or(0) as u16;
    let password = value
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if port == 0 || password.is_empty() {
        return Some(ServiceProbe {
            source: format!("dedicated ({})", lock_path.display()),
            url: None,
            version: None,
            online: false,
            error: Some("daemon.json is missing port or password".into()),
        });
    }
    let rest = RestClient::new(port, password);
    Some(probe(&rest, format!("dedicated ({})", lock_path.display())).await)
}

pub(super) async fn probe_shared_services() -> Vec<ServiceProbe> {
    let mut probes = Vec::new();
    for path in service_state_paths() {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        let Some(url) = value.get("url").and_then(Value::as_str) else {
            continue;
        };
        let Some(password) = value.get("password").and_then(Value::as_str) else {
            continue;
        };
        let rest = RestClient::new_with_url(url, password.to_string());
        probes.push(probe(&rest, format!("shared ({})", path.display())).await);
    }
    probes
}

pub(super) async fn probe(rest: &RestClient, source: String) -> ServiceProbe {
    match rest.info().await {
        Ok(info) => {
            let online = info.version.starts_with('2');
            ServiceProbe {
                source,
                url: Some(rest.base_url().to_string()),
                version: Some(info.version),
                online,
                error: None,
            }
        }
        Err(error) => ServiceProbe {
            source,
            url: Some(rest.base_url().to_string()),
            version: None,
            online: false,
            error: Some(format!("{error:#}")),
        },
    }
}

pub(super) fn service_state_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(state) = std::env::var_os("XDG_STATE_HOME") {
        paths.push(PathBuf::from(state).join("opencode/service.json"));
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        paths.push(home.join(".local/state/opencode/service.json"));
        paths.push(home.join(".config/opencode/service.json"));
    }
    paths
}
