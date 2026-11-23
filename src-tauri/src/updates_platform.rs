use semver::Version;
use std::path::PathBuf;

pub(super) const REPO: &str = "ticosanjr/OpenDesk";
pub(super) const API: &str = "https://api.github.com";
pub(super) const TOKEN_NAMESPACE: &str = "updates";
pub(super) const TOKEN_KEY: &str = "github_token";

pub fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Canonical platform identifier shared with `scripts/release.sh`.
pub fn platform_id() -> String {
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };
    format!("{os}-{}", std::env::consts::ARCH)
}

pub(super) fn portable_asset_name() -> String {
    if cfg!(target_os = "windows") {
        format!("opendesk-{}.exe", platform_id())
    } else {
        format!("opendesk-{}", platform_id())
    }
}

pub(super) fn manifest_matches_platform(manifest: &super::LatestManifest) -> bool {
    manifest.platform.trim().is_empty() || manifest.platform.trim() == platform_id()
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

pub(super) fn compare(current: &str, latest: &str) -> bool {
    let parse = |value: &str| {
        let value = value.trim().trim_start_matches('v');
        Version::parse(value)
    };
    match (parse(current), parse(latest)) {
        (Ok(current), Ok(latest)) => latest > current,
        _ => false,
    }
}
