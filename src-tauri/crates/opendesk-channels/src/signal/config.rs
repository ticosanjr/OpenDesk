//! Signal configuration, persistence and CLI discovery.

use crate::error::AppResult;
use opendesk_storage::repos::settings;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, process::Stdio};
use tokio::process::Command;

const SETTING_KEY: &str = "signal_config";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub account: String,
    #[serde(default = "default_cli")]
    pub cli_path: String,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default = "default_agent")]
    pub agent: String,
}

fn default_cli() -> String {
    default_cli_path()
}

fn default_agent() -> String {
    "build".into()
}

impl Default for SignalConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            account: String::new(),
            cli_path: default_cli(),
            allowlist: Vec::new(),
            workspace_id: None,
            agent: default_agent(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SignalStatus {
    pub config: SignalConfig,
    pub cli_available: bool,
    pub cli_version: Option<String>,
    pub bridge_running: bool,
    pub link_running: bool,
}

pub async fn load(pool: &sqlx::SqlitePool) -> AppResult<SignalConfig> {
    let value = settings::get(pool, SETTING_KEY).await?;
    Ok(value
        .and_then(|value| serde_json::from_value::<SignalConfig>(value).ok())
        .unwrap_or_default())
}

pub async fn save(pool: &sqlx::SqlitePool, config: &SignalConfig) -> AppResult<()> {
    settings::set(pool, SETTING_KEY, &serde_json::to_value(config)?)
        .await
        .map_err(Into::into)
}

pub async fn detect_cli(path: &str) -> Option<String> {
    let output = Command::new(path)
        .arg("--version")
        .env("PATH", signal_path())
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// signal-cli 0.14 requires a recent JVM. AppImages often inherit a minimal
/// PATH, so prepend the common Java installations before spawning it.
pub fn signal_path() -> String {
    let mut paths = Vec::new();
    for candidate in ["/opt/java/current/bin", "/usr/lib/jvm/default-java/bin"] {
        let path = PathBuf::from(candidate);
        if path.is_dir() {
            paths.push(path);
        }
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let sdkman = home.join(".sdkman/candidates/java/current/bin");
        if sdkman.is_dir() {
            paths.push(sdkman);
        }
    }
    if let Some(current) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&current));
    }
    std::env::join_paths(paths)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| "java".into())
}

pub fn default_cli_path() -> String {
    std::env::var("SIGNAL_CLI_PATH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            let candidate = PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join(".local/bin/signal-cli");
            if candidate.is_file() {
                candidate.to_string_lossy().to_string()
            } else {
                "signal-cli".into()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_path_is_never_empty() {
        assert!(!signal_path().is_empty());
    }
}
