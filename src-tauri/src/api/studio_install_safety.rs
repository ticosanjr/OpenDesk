//! Safety boundaries for Studio content installation.
//!
//! A Studio source can be a remote URL, a GitHub repository or a local folder.
//! All three are untrusted input: URLs may target internal services (SSRF),
//! ZIPs may escape the extraction root or expand into a bomb, and local
//! symbolic links may point outside the selected folder.

use crate::{
    api::studio_ops::{InstallItem, collect_entries},
    error::{AppError, AppResult},
};
use reqwest::{
    Url,
    header::{AUTHORIZATION, LOCATION},
    redirect::Policy,
};
use std::{
    io::{Cursor, Read},
    net::{IpAddr, Ipv4Addr},
    path::Path as FsPath,
    time::Duration,
};

pub const MAX_DOWNLOAD_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_ARCHIVE_ENTRIES: usize = 250;
pub const MAX_UNCOMPRESSED_BYTES: u64 = 50 * 1024 * 1024;
pub const MAX_COMPRESSION_RATIO: u64 = 50;

const MAX_REDIRECTS: usize = 5;
const MAX_ENTRY_BYTES: u64 = 2 * 1024 * 1024;

/// Limits applied to ZIP archives and local folders. Kept as a value so tests
/// can exercise the total-size boundary without building a 50 MiB fixture.
#[derive(Debug, Clone, Copy)]
pub struct SafeArchiveLimits {
    pub max_download_bytes: u64,
    pub max_entries: usize,
    pub max_uncompressed_bytes: u64,
    pub max_compression_ratio: u64,
}

impl Default for SafeArchiveLimits {
    fn default() -> Self {
        Self {
            max_download_bytes: MAX_DOWNLOAD_BYTES,
            max_entries: MAX_ARCHIVE_ENTRIES,
            max_uncompressed_bytes: MAX_UNCOMPRESSED_BYTES,
            max_compression_ratio: MAX_COMPRESSION_RATIO,
        }
    }
}

/// Validates an external URL before any request. Production only allows
/// `https`; tests may point at loopback HTTP so a local fixture server works.
pub async fn validate_external_url(raw: &str) -> AppResult<Url> {
    let url =
        Url::parse(raw).map_err(|error| AppError::BadRequest(format!("URL inválida: {error}")))?;
    match url.scheme() {
        "https" => {}
        "http" if cfg!(test) && host_is_loopback(&url) => {}
        scheme => {
            return Err(AppError::BadRequest(format!(
                "esquema '{scheme}' não permitido; use https"
            )));
        }
    }

    let host = url
        .host_str()
        .ok_or_else(|| AppError::BadRequest("URL sem host".into()))?;
    if cfg!(test) && url.scheme() == "http" && host_is_loopback(&url) {
        return Ok(url);
    }
    if let Ok(ip) = host.parse::<IpAddr>() {
        ensure_public_ip(ip)?;
        return Ok(url);
    }

    let port = url.port_or_known_default().unwrap_or(443);
    let mut resolved = false;
    let addresses = tokio::net::lookup_host((host, port))
        .await
        .map_err(|error| {
            AppError::BadRequest(format!("não foi possível resolver {host}: {error}"))
        })?;
    for address in addresses {
        resolved = true;
        ensure_public_ip(address.ip())?;
    }
    if !resolved {
        return Err(AppError::BadRequest(format!(
            "host '{host}' não resolveu para nenhum endereço"
        )));
    }
    Ok(url)
}

fn host_is_loopback(url: &Url) -> bool {
    url.host_str()
        .map(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<IpAddr>()
                    .map(|ip| ip.is_loopback())
                    .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => blocked_v4(ip),
        IpAddr::V6(ip) => {
            if ip.is_loopback() || ip.is_unspecified() {
                return true;
            }
            let segments = ip.segments();
            let unique_local = (segments[0] & 0xfe00) == 0xfc00;
            let link_local = (segments[0] & 0xffc0) == 0xfe80;
            unique_local || link_local || ip.to_ipv4_mapped().map(blocked_v4).unwrap_or(false)
        }
    }
}

fn blocked_v4(ip: Ipv4Addr) -> bool {
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.octets()[0] == 0
}

fn ensure_public_ip(ip: IpAddr) -> AppResult<()> {
    if blocked_ip(ip) {
        return Err(AppError::BadRequest(format!(
            "destino {ip} não é permitido (loopback/rede privada/link-local)"
        )));
    }
    Ok(())
}

/// Downloads a remote source with a hard byte cap and manual redirect
/// validation. Authentication is only sent to the GitHub API host.
pub async fn download_url(url: &str) -> AppResult<Vec<u8>> {
    download_url_auth(url, None, false).await
}

pub async fn download_url_auth(
    url: &str,
    token: Option<&str>,
    authenticated: bool,
) -> AppResult<Vec<u8>> {
    download_url_auth_with(url, token, authenticated, SafeArchiveLimits::default()).await
}

pub async fn download_url_auth_with(
    url: &str,
    token: Option<&str>,
    authenticated: bool,
    limits: SafeArchiveLimits,
) -> AppResult<Vec<u8>> {
    let client = reqwest::Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(45))
        .user_agent("OpenDesk-Studio/1")
        .build()
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let mut current = validate_external_url(url).await?;

    for _ in 0..=MAX_REDIRECTS {
        let mut request = client.get(current.clone());
        if authenticated
            && current
                .host_str()
                .is_some_and(|host| host == "api.github.com")
            && let Some(token) = token
        {
            request = request.header(AUTHORIZATION, format!("Bearer {token}"));
        }
        if authenticated {
            request = request.header("Accept", "application/vnd.github+json");
        }

        let response = request
            .send()
            .await
            .map_err(|error| AppError::BadRequest(format!("download failed: {error}")))?;

        if response.status().is_redirection() {
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| AppError::BadRequest("redirect sem Location".into()))?;
            let next = current
                .join(location)
                .map_err(|error| AppError::BadRequest(format!("redirect inválido: {error}")))?;
            current = validate_external_url(next.as_str()).await?;
            continue;
        }

        if !response.status().is_success() {
            return Err(AppError::BadRequest(format!(
                "download failed with {}",
                response.status()
            )));
        }
        return read_limited(response, limits.max_download_bytes).await;
    }

    Err(AppError::BadRequest("too many redirects".into()))
}

async fn read_limited(mut response: reqwest::Response, maximum: u64) -> AppResult<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum)
    {
        return Err(AppError::BadRequest(format!(
            "download exceeds {} MB",
            maximum / 1024 / 1024
        )));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| AppError::BadRequest(format!("download failed: {error}")))?
    {
        if bytes.len() as u64 + chunk.len() as u64 > maximum {
            return Err(AppError::BadRequest(format!(
                "download exceeds {} MB",
                maximum / 1024 / 1024
            )));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub fn collect_zip(
    bytes: &[u8],
    items: &mut Vec<InstallItem>,
    warnings: &mut Vec<String>,
) -> AppResult<()> {
    collect_zip_with(bytes, items, warnings, SafeArchiveLimits::default())
}

pub fn collect_zip_with(
    bytes: &[u8],
    items: &mut Vec<InstallItem>,
    warnings: &mut Vec<String>,
    limits: SafeArchiveLimits,
) -> AppResult<()> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes.to_vec()))
        .map_err(|error| AppError::BadRequest(format!("invalid zip: {error}")))?;
    if archive.len() > limits.max_entries {
        return Err(AppError::BadRequest(format!(
            "zip contains more than {} entries",
            limits.max_entries
        )));
    }

    let mut entries = Vec::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|error| AppError::BadRequest(format!("invalid zip entry: {error}")))?;
        if file.is_dir() {
            continue;
        }
        let Some(enclosed) = file.enclosed_name() else {
            warnings.push(format!(
                "ignored unsafe path {}",
                file.name()
                    .map(|name| name.into_owned())
                    .unwrap_or_else(|_| "invalid entry".into())
            ));
            continue;
        };
        let name = enclosed.to_string_lossy().replace('\\', "/");
        if file
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            warnings.push(format!("ignored symlink {name}"));
            continue;
        }
        if file.size() > MAX_ENTRY_BYTES {
            warnings.push(format!("ignored oversized entry {name}"));
            continue;
        }
        if file.size() > 0
            && file.compressed_size() > 0
            && file.size() / file.compressed_size() > limits.max_compression_ratio
        {
            return Err(AppError::BadRequest(format!(
                "zip entry {name} exceeds the compression ratio limit"
            )));
        }
        let remaining = limits.max_uncompressed_bytes.saturating_sub(total);
        if remaining == 0 {
            return Err(AppError::BadRequest(
                "zip exceeds the uncompressed size limit".into(),
            ));
        }
        let mut limited = (&mut file).take(remaining + 1);
        let mut content = Vec::new();
        limited
            .read_to_end(&mut content)
            .map_err(|error| AppError::BadRequest(format!("invalid zip entry: {error}")))?;
        if content.len() as u64 > remaining {
            return Err(AppError::BadRequest(
                "zip exceeds the uncompressed size limit".into(),
            ));
        }
        total = total.saturating_add(content.len() as u64);
        let Ok(text) = String::from_utf8(content) else {
            warnings.push(format!("ignored non-text entry {name}"));
            continue;
        };
        entries.push((name, text));
    }

    collect_entries(entries, items, warnings, None);
    Ok(())
}

pub fn collect_local_dir(
    path: &FsPath,
    items: &mut Vec<InstallItem>,
    warnings: &mut Vec<String>,
) -> AppResult<()> {
    collect_local_dir_with(path, items, warnings, SafeArchiveLimits::default())
}

pub fn collect_local_dir_with(
    path: &FsPath,
    items: &mut Vec<InstallItem>,
    warnings: &mut Vec<String>,
    limits: SafeArchiveLimits,
) -> AppResult<()> {
    let root = path
        .canonicalize()
        .map_err(|error| AppError::BadRequest(format!("invalid local path: {error}")))?;
    let mut entries = Vec::new();
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(&root).max_depth(8) {
        let Ok(entry) = entry else { continue };
        if entry.file_type().is_symlink() || !entry.file_type().is_file() {
            continue;
        }
        let Ok(canonical) = entry.path().canonicalize() else {
            continue;
        };
        if !canonical.starts_with(&root) {
            warnings.push(format!(
                "ignored path outside the selected folder: {}",
                entry.path().display()
            ));
            continue;
        }
        if entries.len() >= limits.max_entries {
            return Err(AppError::BadRequest(format!(
                "folder contains more than {} files",
                limits.max_entries
            )));
        }
        let Ok(relative) = canonical.strip_prefix(&root) else {
            continue;
        };
        let name = relative.to_string_lossy().replace('\\', "/");
        if !name.to_lowercase().ends_with(".md") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&canonical) else {
            continue;
        };
        total = total.saturating_add(content.len() as u64);
        if total > limits.max_uncompressed_bytes {
            return Err(AppError::BadRequest(
                "folder exceeds the content size limit".into(),
            ));
        }
        entries.push((name, content));
    }
    collect_entries(entries, items, warnings, Some(&root));
    Ok(())
}

#[cfg(test)]
#[path = "studio_install_safety_tests.rs"]
mod tests;
