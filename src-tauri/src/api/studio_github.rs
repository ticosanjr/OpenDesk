//! GitHub source resolution for Studio content installation.

use crate::{
    api::studio_install_safety,
    error::{AppError, AppResult},
};
use serde_json::Value;

pub(crate) async fn download_github(source: &str) -> AppResult<Vec<u8>> {
    let source = source.trim_start_matches("github:");
    let (repo_path, requested_ref) = match source.split_once('@') {
        Some((repo, reference)) => (repo, Some(reference.to_string())),
        None => (source, None),
    };
    let mut parts = repo_path.splitn(3, '/');
    let owner = parts.next().unwrap_or_default();
    let repo = parts.next().unwrap_or_default();
    if owner.is_empty() || repo.is_empty() {
        return Err(AppError::BadRequest(
            "GitHub source must be owner/repo, owner/repo@ref or github:owner/repo".into(),
        ));
    }
    let token = github_token().await;
    let reference = match requested_ref {
        Some(reference) => reference,
        None => github_default_branch(owner, repo, token.as_deref()).await?,
    };

    let (url, authenticated) = match &token {
        Some(_) => (
            format!("https://api.github.com/repos/{owner}/{repo}/zipball/{reference}"),
            true,
        ),
        None => (
            format!("https://codeload.github.com/{owner}/{repo}/zip/refs/heads/{reference}"),
            false,
        ),
    };
    let bytes = studio_install_safety::download_url_auth(&url, token.as_deref(), authenticated).await.map_err(|_| {
        AppError::BadRequest(format!(
            "could not download GitHub repository {owner}/{repo}@{reference}; check the ref, visibility and token"
        ))
    })?;
    Ok(bytes)
}

pub(crate) async fn github_token() -> Option<String> {
    for key in ["GITHUB_TOKEN", "GH_TOKEN", "OPENDESK_GITHUB_TOKEN"] {
        if let Ok(value) = std::env::var(key)
            && !value.trim().is_empty()
        {
            return Some(value.trim().to_string());
        }
    }
    let output = tokio::process::Command::new("gh")
        .args(["auth", "token"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!value.is_empty()).then_some(value)
}

pub(crate) async fn github_default_branch(
    owner: &str,
    repo: &str,
    token: Option<&str>,
) -> AppResult<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("OpenDesk-Studio/1")
        .build()
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let mut request = client
        .get(format!("https://api.github.com/repos/{owner}/{repo}"))
        .header("Accept", "application/vnd.github+json");
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request
        .send()
        .await
        .map_err(|error| AppError::BadRequest(format!("GitHub API failed: {error}")))?;
    if !response.status().is_success() {
        // Public repositories with a rate limit still work through codeload
        // with the conventional main branch.
        return Ok("main".into());
    }
    let value: Value = response
        .json()
        .await
        .map_err(|error| AppError::BadRequest(format!("invalid GitHub response: {error}")))?;
    Ok(value
        .get("default_branch")
        .and_then(Value::as_str)
        .unwrap_or("main")
        .to_string())
}
