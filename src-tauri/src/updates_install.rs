use crate::error::{AppError, AppResult};
use serde_json::Value;
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use super::platform::{API, REPO, portable_asset_name};
use super::{LatestManifest, UpdateAsset};

pub(super) fn select_asset<'a>(
    manifest: &'a LatestManifest,
    installer: &str,
) -> Option<&'a UpdateAsset> {
    let portable = portable_asset_name();
    let matches = |asset: &UpdateAsset| match installer {
        "appimage" => asset.name.ends_with(".AppImage"),
        "system" => asset.name.ends_with(".deb"),
        "development" => false,
        _ => asset.name == portable,
    };
    manifest.assets.iter().find(|asset| matches(asset))
}

pub(super) async fn find_asset_id(
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

pub(super) fn verify_sha256(digest: &str, expected: &str) -> bool {
    !expected.trim().is_empty() && digest.eq_ignore_ascii_case(expected.trim())
}

pub(super) fn write_executable(path: &Path, bytes: &[u8]) -> AppResult<()> {
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
