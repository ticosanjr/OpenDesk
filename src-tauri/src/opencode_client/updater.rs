use crate::error::{AppError, AppResult};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::Duration;

const RELEASES_URL: &str = "https://api.github.com/repos/anomalyco/opencode/releases?per_page=50";

/// A stable OpenCode release that OpenDesk is allowed to install. Only V2
/// releases are considered: the repository also publishes V1 tags and
/// auxiliary `process-daemon-*` tags, and `/releases/latest` still points at a
/// V1 build.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeRelease {
    pub version: String,
    pub tag: String,
    pub name: String,
    pub notes: Option<String>,
    pub url: String,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenCodeUpdateCheck {
    pub current: Option<String>,
    pub latest: Option<String>,
    pub update_available: bool,
    pub release: Option<OpenCodeRelease>,
    pub checked_at: String,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReleaseDto {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    draft: bool,
}

pub fn http_client() -> AppResult<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(format!("OpenDesk/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| AppError::Internal(format!("could not build the HTTP client: {error}")))
}

/// Fetch every stable OpenCode V2 release and pick the highest semver.
pub async fn fetch_latest(client: &reqwest::Client) -> AppResult<OpenCodeRelease> {
    let mut request = client.get(RELEASES_URL);
    if let Ok(token) = std::env::var("GITHUB_TOKEN") {
        if !token.trim().is_empty() {
            request = request.bearer_auth(token);
        }
    }

    let response = request
        .send()
        .await
        .map_err(|error| AppError::Daemon(format!("could not reach GitHub: {error}")))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::Daemon(format!(
            "GitHub releases request failed ({status}). Set GITHUB_TOKEN if you are rate limited."
        )));
    }

    let releases: Vec<ReleaseDto> = serde_json::from_str(&body)
        .map_err(|error| AppError::Internal(format!("invalid GitHub releases payload: {error}")))?;

    select_latest(releases).ok_or_else(|| {
        AppError::Daemon(
            "no stable OpenCode V2 release was found in the GitHub releases list".into(),
        )
    })
}

fn select_latest<I>(releases: I) -> Option<OpenCodeRelease>
where
    I: IntoIterator<Item = ReleaseDto>,
{
    let mut best: Option<(Version, OpenCodeRelease)> = None;

    for release in releases {
        if release.draft || release.prerelease {
            continue;
        }
        let Some(version) = parse_tag(&release.tag_name) else {
            continue;
        };
        if version.major != 2 {
            continue;
        }
        let candidate = OpenCodeRelease {
            version: version.to_string(),
            tag: release.tag_name.clone(),
            name: release
                .name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or_else(|| release.tag_name.clone()),
            notes: release.body,
            url: release.html_url.unwrap_or_default(),
            published_at: release.published_at,
        };
        match &best {
            Some((best_version, _)) if *best_version >= version => {}
            _ => best = Some((version, candidate)),
        }
    }

    best.map(|(_, release)| release)
}

fn parse_tag(tag: &str) -> Option<Version> {
    let tag = tag.trim();
    let tag = tag.strip_prefix('v').unwrap_or(tag);
    let tag = tag.split(['-', '+']).next().unwrap_or(tag);
    Version::parse(tag).ok()
}

pub fn update_available(current: Option<&str>, latest: &str) -> bool {
    let Some(latest) = parse_version(latest) else {
        return false;
    };
    match current.and_then(parse_version) {
        Some(current) => latest > current,
        None => true,
    }
}

fn parse_version(value: &str) -> Option<Version> {
    let value = value.trim();
    let value = value.strip_prefix('v').unwrap_or(value);
    let value = value.split_whitespace().next().unwrap_or(value);
    Version::parse(value).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, prerelease: bool, draft: bool) -> ReleaseDto {
        ReleaseDto {
            tag_name: tag.to_string(),
            name: Some(tag.to_string()),
            body: None,
            html_url: Some(format!(
                "https://github.com/anomalyco/opencode/releases/tag/{tag}"
            )),
            published_at: None,
            prerelease,
            draft,
        }
    }

    #[test]
    fn selects_the_highest_stable_v2_release() {
        let selected = select_latest(vec![
            release("v2.2.4", false, false),
            release("v1.19.5", false, false),
            release("v2.2.5", false, false),
            release("process-daemon-v0.5.0", false, false),
            release("v2.3.0-rc.1", true, false),
            release("v2.4.0", false, true),
        ])
        .expect("a release");

        assert_eq!(selected.version, "2.2.5");
        assert_eq!(selected.tag, "v2.2.5");
    }

    #[test]
    fn ignores_non_semver_and_v1_tags() {
        assert!(select_latest(vec![release("nightly", false, false)]).is_none());
        assert!(select_latest(vec![release("v1.19.5", false, false)]).is_none());
    }

    #[test]
    fn compares_versions() {
        assert!(update_available(Some("2.2.5"), "2.2.6"));
        assert!(!update_available(Some("2.2.5"), "2.2.5"));
        assert!(!update_available(Some("2.2.5"), "v2.2.4"));
        assert!(!update_available(Some("2.2.5"), "not-a-version"));
        assert!(update_available(None, "2.2.5"));
    }

    #[test]
    fn parses_tags_with_prefixes() {
        assert_eq!(parse_tag("v2.2.5").unwrap().to_string(), "2.2.5");
        assert_eq!(parse_tag("2.2.5").unwrap().to_string(), "2.2.5");
        assert!(parse_tag("process-daemon-v0.5.0").is_none());
    }
}
