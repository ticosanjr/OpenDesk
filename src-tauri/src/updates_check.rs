use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    secrets::SecretStore,
    update_signing,
};
use semver::Version;
use serde_json::Value;
use std::{
    process::{Command, Stdio},
    time::Duration,
};

use super::platform::{
    API, REPO, TOKEN_KEY, TOKEN_NAMESPACE, compare, current_version, installer_kind,
};
use super::{LatestManifest, UpdateInfo};

pub fn token(secrets: &SecretStore) -> Option<String> {
    if let Ok(Some(value)) = secrets.get(TOKEN_NAMESPACE, TOKEN_KEY)
        && !value.trim().is_empty()
    {
        return Some(value.trim().to_string());
    }
    for key in ["OPENDESK_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"] {
        if let Ok(value) = std::env::var(key)
            && !value.trim().is_empty()
        {
            return Some(value.trim().to_string());
        }
    }
    // Fall back to the authenticated gh CLI when the user already uses it.
    let output = Command::new("gh")
        .args(["auth", "token"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

pub fn save_token(state: &AppState, value: &str) -> AppResult<()> {
    let value = value.trim();
    if value.is_empty() {
        state.secrets.delete(TOKEN_NAMESPACE, TOKEN_KEY)?;
    } else {
        state.secrets.set(TOKEN_NAMESPACE, TOKEN_KEY, value)?;
    }
    Ok(())
}

pub async fn check(state: &AppState) -> UpdateInfo {
    let current = current_version();
    let token = token(&state.secrets);
    let installer = installer_kind().to_string();
    let checked_at = chrono::Utc::now().to_rfc3339();

    let Some(token) = token else {
        return UpdateInfo {
            current,
            latest: None,
            update_available: false,
            token_configured: false,
            installer,
            checked_at,
            error: Some(
                "Este repositório é privado. Configure um token do GitHub (fine-grained, Contents: Read e Metadata: Read) em Atualizações, ou autentique o gh CLI.".into(),
            ),
        };
    };

    match fetch_latest(&token).await {
        Ok(manifest) => {
            let available = compare(&current, &manifest.version);
            UpdateInfo {
                current,
                latest: Some(manifest),
                update_available: available,
                token_configured: true,
                installer,
                checked_at,
                error: None,
            }
        }
        Err(error) => UpdateInfo {
            current,
            latest: None,
            update_available: false,
            token_configured: true,
            installer,
            checked_at,
            error: Some(format!("{error:#}")),
        },
    }
}

pub(super) async fn fetch_latest(token: &str) -> AppResult<LatestManifest> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(format!("OpenDesk/{}", current_version()))
        .build()
        .map_err(|error| AppError::Internal(format!("could not build the HTTP client: {error}")))?;

    let response = client
        .get(format!("{API}/repos/{REPO}/releases/latest"))
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| AppError::Daemon(format!("could not reach GitHub: {error}")))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err(AppError::Daemon(format!(
            "nenhuma GitHub Release publicada em {REPO} (as tags existem, mas a release ainda não foi criada)."
        )));
    }
    if !status.is_success() {
        return Err(AppError::Daemon(format!(
            "GitHub respondeu {status}. Verifique se o token tem acesso a {REPO}."
        )));
    }

    let release: Value = serde_json::from_str(&body)
        .map_err(|error| AppError::Internal(format!("invalid GitHub release: {error}")))?;
    let tag = release
        .get("tag_name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let notes = release
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let release_assets = release
        .get("assets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let asset_id = |name: &str| {
        release_assets
            .iter()
            .find(|asset| asset.get("name").and_then(Value::as_str) == Some(name))
            .and_then(|asset| asset.get("id").and_then(Value::as_u64))
    };

    // A release is only trusted when it publishes both latest.json and its
    // detached Ed25519 signature. Old unsigned releases are refused with a
    // clear message instead of silently accepting weak checksums.
    let manifest_id = asset_id("latest.json").ok_or_else(|| {
        AppError::Daemon(format!(
            "a release {tag} não publica latest.json; atualize manualmente pela página de releases."
        ))
    })?;
    let signature_id = asset_id("latest.json.sig").ok_or_else(|| {
        AppError::Daemon(format!(
            "a release {tag} não tem latest.json.sig; o OpenDesk recusa updates sem assinatura."
        ))
    })?;
    let manifest_bytes = download_asset(&client, token, manifest_id).await?;
    let signature_bytes = download_asset(&client, token, signature_id).await?;
    let signature = std::str::from_utf8(&signature_bytes)
        .map_err(|_| AppError::Daemon("latest.json.sig não é UTF-8".into()))?;
    update_signing::verify_manifest(&manifest_bytes, signature)
        .map_err(|error| AppError::Daemon(format!("assinatura do update inválida: {error}")))?;

    let mut manifest: LatestManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| AppError::Internal(format!("latest.json inválido: {error}")))?;
    if manifest.tag.trim().is_empty() {
        manifest.tag = tag;
    }
    if manifest.notes.trim().is_empty() {
        manifest.notes = notes;
    }
    if Version::parse(manifest.version.trim().trim_start_matches('v')).is_err() {
        return Err(AppError::Daemon(format!(
            "latest.json tem versão inválida ({})",
            manifest.version
        )));
    }
    Ok(manifest)
}

pub(super) async fn download_asset(
    client: &reqwest::Client,
    token: &str,
    asset_id: u64,
) -> AppResult<Vec<u8>> {
    let response = client
        .get(format!("{API}/repos/{REPO}/releases/assets/{asset_id}"))
        .bearer_auth(token)
        .header("Accept", "application/octet-stream")
        .send()
        .await
        .map_err(|error| AppError::Daemon(format!("asset download failed: {error}")))?;
    let status = response.status();
    if !status.is_success() {
        return Err(AppError::Daemon(format!(
            "asset download failed with {status}"
        )));
    }
    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|error| AppError::Daemon(format!("asset download failed: {error}")))
}
