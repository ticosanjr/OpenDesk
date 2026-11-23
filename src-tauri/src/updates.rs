//! GitHub release checks, signature verification and self-update installation.
//! The public surface stays flat (`crate::updates::*`); the work is split into
//! platform, check, download and install concerns.

use serde::{Deserialize, Serialize};

#[path = "updates_check.rs"]
mod check;
#[path = "updates_download.rs"]
mod download;
#[path = "updates_install.rs"]
mod install;
#[path = "updates_platform.rs"]
mod platform;

pub use check::*;
pub use download::*;
pub(crate) use install::spawn_replacer;
pub(crate) use platform::running_image;
pub use platform::*;

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

#[cfg(test)]
#[path = "updates_tests.rs"]
mod tests;
