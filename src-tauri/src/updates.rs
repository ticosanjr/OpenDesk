use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    secrets::SecretStore,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

const REPO: &str = "ticosanjr/OpenDesk";
const API: &str = "https://api.github.com";
const TOKEN_NAMESPACE: &str = "updates";
const TOKEN_KEY: &str = "github_token";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAsset {
    pub name: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestManifest {
    pub version: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub published_at: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub assets: Vec<UpdateAsset>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: Option<LatestManifest>,
    pub update_available: bool,
    pub token_configured: bool,
    pub installer: String,
    pub checked_at: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadResult {
    pub ok: bool,
    pub action: String,
    pub path: Option<String>,
    pub message: String,
}

pub fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// How this install can be updated. The AppImage and the portable binary can
/// replace themselves; a system package must go through the package manager.
pub fn installer_kind() -> &'static str {
    if cfg!(debug_assertions) {
        return "development";
    }
    if let Some(path) = running_image() {
        let value = path.to_string_lossy().to_lowercase();
        if value.ends_with(".appimage") {
            return "appimage";
        }
        if value.contains("/target/debug/") || value.contains("/target/release/deps/") {
            return "development";
        }
    }
    #[cfg(unix)]
    {
        let exe = std::env::current_exe().unwrap_or_default();
        let value = exe.to_string_lossy();
        if value.starts_with("/usr/bin/") || value.starts_with("/usr/local/bin/") {
            return "system";
        }
    }
    "portable"
}

pub(crate) fn running_image() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE")
        .map(PathBuf::from)
        .or_else(|| std::env::current_exe().ok())
}

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

fn compare(current: &str, latest: &str) -> bool {
    let parse = |value: &str| {
        let value = value.trim().trim_start_matches('v');
        Version::parse(value)
    };
    match (parse(current), parse(latest)) {
        (Ok(current), Ok(latest)) => latest > current,
        _ => false,
    }
}

async fn fetch_latest(token: &str) -> AppResult<LatestManifest> {
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

    // Prefer the signed manifest asset uploaded by scripts/release.sh. It has
    // the SHA256 for every artifact; the raw release assets do not.
    let mut manifest_from_asset = None;
    if let Some(assets) = release.get("assets").and_then(Value::as_array)
        && let Some(asset) = assets
            .iter()
            .find(|asset| asset.get("name").and_then(Value::as_str) == Some("latest.json"))
        && let Some(asset_id) = asset.get("id").and_then(Value::as_u64)
    {
        let bytes = download_asset(&client, token, asset_id).await?;
        manifest_from_asset = serde_json::from_slice::<LatestManifest>(&bytes).ok();
    }

    if let Some(mut manifest) = manifest_from_asset {
        if manifest.notes.trim().is_empty() {
            manifest.notes = notes;
        }
        return Ok(manifest);
    }

    // Fallback for releases created before latest.json existed: build a
    // manifest from the release assets, but without checksums.
    let version = tag.trim_start_matches('v').to_string();
    if Version::parse(&version).is_err() {
        return Err(AppError::Daemon(format!(
            "a release mais recente tem tag inválida ({tag})"
        )));
    }
    let assets = release
        .get("assets")
        .and_then(Value::as_array)
        .map(|assets| {
            assets
                .iter()
                .filter_map(|asset| {
                    Some(UpdateAsset {
                        name: asset.get("name")?.as_str()?.to_string(),
                        size: asset.get("size").and_then(Value::as_u64).unwrap_or(0),
                        sha256: String::new(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(LatestManifest {
        version,
        tag,
        repo: REPO.to_string(),
        platform: "linux-x86_64".into(),
        published_at: release
            .get("published_at")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        notes,
        assets,
    })
}

async fn download_asset(
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

pub async fn download_and_stage(state: &AppState) -> AppResult<DownloadResult> {
    let installer = installer_kind();
    if installer == "development" {
        return Ok(DownloadResult {
            ok: false,
            action: "unsupported".into(),
            path: None,
            message: "Esta é uma build de desenvolvimento; a auto-atualização é desabilitada. Rode `scripts/release.sh` para gerar a versão instalável.".into(),
        });
    }

    let Some(token) = token(&state.secrets) else {
        return Err(AppError::BadRequest(
            "Configure um token do GitHub para baixar a atualização.".into(),
        ));
    };

    let manifest = fetch_latest(&token).await?;
    if !compare(&current_version(), &manifest.version) {
        return Ok(DownloadResult {
            ok: false,
            action: "up-to-date".into(),
            path: None,
            message: format!("O OpenDesk já está na versão {}.", manifest.version),
        });
    }

    let asset = select_asset(&manifest, installer).ok_or_else(|| {
        AppError::BadRequest(format!(
            "a release {} não publicou um asset compatível com {}",
            manifest.version, installer
        ))
    })?;

    if asset.sha256.trim().is_empty() {
        return Err(AppError::BadRequest(format!(
            "o asset {} não tem SHA256 no latest.json; publique uma release com scripts/release.sh para habilitar a verificação.",
            asset.name
        )));
    }
    let asset_name = asset.name.clone();
    let expected_sha = asset.sha256.clone();

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .user_agent(format!("OpenDesk/{}", current_version()))
        .build()
        .map_err(|error| AppError::Internal(format!("could not build the HTTP client: {error}")))?;
    let asset_id = find_asset_id(&client, &token, &manifest, &asset_name).await?;
    let bytes = download_asset(&client, &token, asset_id).await?;

    let digest = Sha256::digest(&bytes);
    let digest = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if !verify_sha256(&digest, &expected_sha) {
        return Err(AppError::Internal(format!(
            "SHA256 mismatch for {}: expected {}, got {}",
            asset_name, expected_sha, digest
        )));
    }

    let staging = state.paths.data_dir.join("updates").join(&manifest.version);
    std::fs::create_dir_all(&staging)?;
    let staged = staging.join(&asset_name);
    write_executable(&staged, &bytes)?;

    if installer == "system" {
        let download_dir = directories::UserDirs::new()
            .and_then(|dirs| dirs.download_dir().map(Path::to_path_buf))
            .unwrap_or_else(|| state.paths.data_dir.clone());
        std::fs::create_dir_all(&download_dir)?;
        let target = download_dir.join(&asset_name);
        std::fs::copy(&staged, &target)?;
        return Ok(DownloadResult {
            ok: true,
            action: "manual-package".into(),
            path: Some(target.to_string_lossy().to_string()),
            message: format!(
                "Pacote salvo em {}. Instale com: sudo dpkg -i {}",
                target.display(),
                target.display()
            ),
        });
    }

    let target = running_image().ok_or_else(|| {
        AppError::Internal("não foi possível resolver o caminho do OpenDesk em execução".into())
    })?;
    if !target.is_file() {
        return Err(AppError::BadRequest(format!(
            "o arquivo em execução ({}) não existe mais; baixe manualmente a release.",
            target.display()
        )));
    }
    let backup = target.with_file_name(format!(
        "{}.bak",
        target
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "opendesk".into())
    ));
    if backup.exists() {
        let _ = std::fs::remove_file(&backup);
    }
    std::fs::copy(&target, &backup)?;
    spawn_replacer(&staged, &target, std::process::id())?;

    Ok(DownloadResult {
        ok: true,
        action: "restart-required".into(),
        path: Some(target.to_string_lossy().to_string()),
        message: format!(
            "Atualização {} baixada e verificada. Encerre o OpenDesk (inclusive pelo ícone da bandeja); o binário será substituído e o app reaberto automaticamente. Backup: {}.",
            manifest.version,
            backup.display()
        ),
    })
}

fn select_asset<'a>(manifest: &'a LatestManifest, installer: &str) -> Option<&'a UpdateAsset> {
    let matches = |asset: &UpdateAsset| match installer {
        "appimage" => asset.name.ends_with(".AppImage"),
        "system" => asset.name.ends_with(".deb"),
        "development" => false,
        _ => asset.name == "opendesk-linux-x86_64" || !asset.name.contains('.'),
    };
    manifest.assets.iter().find(|asset| matches(asset))
}

async fn find_asset_id(
    client: &reqwest::Client,
    token: &str,
    manifest: &LatestManifest,
    asset_name: &str,
) -> AppResult<u64> {
    let response = client
        .get(format!("{API}/repos/{REPO}/releases/tags/{}", manifest.tag))
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| AppError::Daemon(format!("could not reach GitHub: {error}")))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::Daemon(format!(
            "could not read release {}: {status}",
            manifest.tag
        )));
    }
    let value: Value = serde_json::from_str(&body)
        .map_err(|error| AppError::Internal(format!("invalid GitHub release: {error}")))?;
    value
        .get("assets")
        .and_then(Value::as_array)
        .and_then(|assets| {
            assets
                .iter()
                .find(|asset| asset.get("name").and_then(Value::as_str) == Some(asset_name))
        })
        .and_then(|asset| asset.get("id").and_then(Value::as_u64))
        .ok_or_else(|| {
            AppError::BadRequest(format!("asset {asset_name} não encontrado na release"))
        })
}

fn verify_sha256(digest: &str, expected: &str) -> bool {
    !expected.trim().is_empty() && digest.eq_ignore_ascii_case(expected.trim())
}

fn write_executable(path: &Path, bytes: &[u8]) -> AppResult<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// Write a small detached helper that waits for the current process to exit,
/// then replaces the binary and relaunches it. This avoids corrupting a running
/// AppImage and keeps the old file as `.bak` for a manual rollback.
pub(crate) fn spawn_replacer(staged: &Path, target: &Path, pid: u32) -> AppResult<()> {
    let script_path = target.with_extension(format!("update-{pid}.sh"));
    let launch = format!("nohup \"{}\" >/dev/null 2>&1 &", target.display());
    let script = format!(
        "#!/bin/sh\nwhile kill -0 {pid} 2>/dev/null; do sleep 0.5; done\ncp -f \"{}\" \"{}.new\" && chmod 755 \"{}.new\" && mv -f \"{}.new\" \"{}\" && rm -f \"{}\"\n{launch}\nrm -f \"$0\"\n",
        staged.display(),
        target.display(),
        target.display(),
        target.display(),
        target.display(),
        staged.display(),
    );
    let mut file = std::fs::File::create(&script_path)?;
    file.write_all(script.as_bytes())?;
    file.sync_all()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script_path, std::fs::Permissions::from_mode(0o700))?;
    }

    Command::new("setsid")
        .arg("sh")
        .arg(&script_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            AppError::Internal(format!("could not start the update helper: {error}"))
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(assets: Vec<(&str, &str)>) -> LatestManifest {
        LatestManifest {
            version: "0.7.0".into(),
            tag: "v0.7.0".into(),
            repo: REPO.into(),
            platform: "linux-x86_64".into(),
            published_at: String::new(),
            notes: String::new(),
            assets: assets
                .into_iter()
                .map(|(name, sha)| UpdateAsset {
                    name: name.into(),
                    size: 0,
                    sha256: sha.into(),
                })
                .collect(),
        }
    }

    #[test]
    fn selects_assets_per_installer() {
        let value = manifest(vec![
            ("OpenDesk_0.7.0_amd64.AppImage", "a"),
            ("OpenDesk_0.7.0_amd64.deb", "b"),
            ("opendesk-linux-x86_64", "c"),
        ]);
        assert_eq!(
            select_asset(&value, "appimage").unwrap().name,
            "OpenDesk_0.7.0_amd64.AppImage"
        );
        assert_eq!(
            select_asset(&value, "system").unwrap().name,
            "OpenDesk_0.7.0_amd64.deb"
        );
        assert_eq!(
            select_asset(&value, "portable").unwrap().name,
            "opendesk-linux-x86_64"
        );
    }

    #[test]
    fn compares_versions_with_and_without_prefix() {
        assert!(compare("0.6.0", "0.7.0"));
        assert!(compare("0.6.0", "v0.7.0"));
        assert!(!compare("0.7.0", "0.7.0"));
        assert!(!compare("0.7.0", "0.6.9"));
        assert!(!compare("dev", "0.7.0"));
    }

    #[test]
    fn development_build_skips_asset_selection() {
        let value = manifest(vec![("opendesk-linux-x86_64", "c")]);
        assert!(select_asset(&value, "development").is_none());
    }

    #[test]
    fn verifies_sha256_case_insensitively() {
        assert!(verify_sha256("abc123", "ABC123"));
        assert!(verify_sha256("abc123", " abc123 "));
        assert!(!verify_sha256("abc123", "abc124"));
        assert!(!verify_sha256("abc123", ""));
    }

    #[test]
    fn parses_latest_manifest() {
        let raw = r#"{
            "version": "0.6.0",
            "tag": "v0.6.0",
            "repo": "ticosanjr/OpenDesk",
            "platform": "linux-x86_64",
            "published_at": "2026-11-06T00:00:00Z",
            "notes": "release",
            "assets": [
                {"name": "OpenDesk_0.6.0_amd64.AppImage", "size": 10, "sha256": "aa"}
            ]
        }"#;
        let manifest: LatestManifest = serde_json::from_str(raw).expect("manifest");
        assert_eq!(manifest.version, "0.6.0");
        assert_eq!(manifest.assets.len(), 1);
        assert_eq!(
            select_asset(&manifest, "appimage").unwrap().name,
            "OpenDesk_0.6.0_amd64.AppImage"
        );
    }

    #[test]
    fn exposes_the_build_version_and_development_installer() {
        assert_eq!(current_version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(installer_kind(), "development");
    }

    #[test]
    fn selects_nothing_for_unknown_installers_or_empty_manifests() {
        let empty = manifest(Vec::new());
        assert!(select_asset(&empty, "appimage").is_none());
        let value = manifest(vec![("opendesk-linux-x86_64", "c")]);
        assert_eq!(
            select_asset(&value, "unknown").expect("fallback").name,
            "opendesk-linux-x86_64"
        );
        assert!(select_asset(&value, "portable").is_some());
    }
}
