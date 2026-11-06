use crate::error::{AppError, AppResult};
use chrono::Utc;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct FileSnapshot {
    pub content: String,
    pub sha256: String,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

pub fn read_text(path: &Path) -> AppResult<Option<FileSnapshot>> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(Some(FileSnapshot {
            sha256: sha256_hex(content.as_bytes()),
            content,
        })),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(AppError::Io(error)),
    }
}

pub fn read_required(path: &Path) -> AppResult<FileSnapshot> {
    read_text(path)?.ok_or_else(|| AppError::NotFound(format!("file {}", path.display())))
}

/// Atomic-ish write with an optimistic concurrency check and a backup.
///
/// `expected_sha256` is the hash the caller read before editing. A mismatch is
/// a 409 conflict, never a silent overwrite of an external editor's change.
pub fn write_text_atomic(
    path: &Path,
    content: &str,
    expected_sha256: Option<&str>,
    backups_dir: &Path,
) -> AppResult<String> {
    let existing = read_text(path)?;

    if let Some(expected) = expected_sha256 {
        match &existing {
            Some(snapshot) if snapshot.sha256 == expected => {}
            Some(_) => {
                return Err(AppError::Conflict(format!(
                    "{} changed on disk; reload before saving",
                    path.display()
                )));
            }
            None => {
                return Err(AppError::Conflict(format!(
                    "{} no longer exists; reload before saving",
                    path.display()
                )));
            }
        }
    }

    if let Some(snapshot) = &existing {
        backup_file(path, snapshot, backups_dir)?;
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let temp_path = path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "opendesk".to_string()),
        Uuid::new_v4().simple()
    ));

    std::fs::write(&temp_path, content.as_bytes())?;
    {
        let file = std::fs::OpenOptions::new().read(true).open(&temp_path)?;
        file.sync_all()?;
    }

    #[cfg(windows)]
    if path.exists() {
        std::fs::remove_file(path)?;
    }

    std::fs::rename(&temp_path, path)?;
    Ok(sha256_hex(content.as_bytes()))
}

fn backup_file(path: &Path, snapshot: &FileSnapshot, backups_dir: &Path) -> AppResult<()> {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "config".to_string());
    let stamp = Utc::now().format("%Y%m%dT%H%M%S%.3fZ");
    let backup_dir = backups_dir.join(format!("{stamp}-{}", Uuid::new_v4().simple()));
    std::fs::create_dir_all(&backup_dir)?;
    std::fs::write(backup_dir.join(file_name), snapshot.content.as_bytes())?;
    Ok(())
}

pub fn delete_path(path: &Path) -> AppResult<()> {
    if !path.exists() {
        return Err(AppError::NotFound(format!("path {}", path.display())));
    }
    if path.is_dir() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn validate_slug(slug: &str, allow_nested: bool) -> AppResult<String> {
    let value = slug.trim();
    if value.is_empty() {
        return Err(AppError::BadRequest("slug is required".into()));
    }
    if value.starts_with('/') || value.contains('\\') || value.contains("..") {
        return Err(AppError::BadRequest(format!("invalid slug '{slug}'")));
    }

    let segments: Vec<&str> = value.split('/').collect();
    if !allow_nested && segments.len() != 1 {
        return Err(AppError::BadRequest(
            "nested slugs are not allowed here".into(),
        ));
    }

    for segment in segments {
        if segment.is_empty()
            || !segment
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(AppError::BadRequest(format!(
                "invalid slug '{slug}'; use lowercase letters, digits, '-' or '_'"
            )));
        }
    }

    Ok(value.to_string())
}

pub fn validate_skill_slug(slug: &str) -> AppResult<String> {
    let value = validate_slug(slug, false)?;
    let valid = value
        .split('-')
        .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric()));
    if !valid {
        return Err(AppError::BadRequest(format!(
            "invalid skill id '{slug}'; expected kebab-case"
        )));
    }
    Ok(value)
}

pub fn validate_relative_file(path: &str) -> AppResult<String> {
    let value = path.trim();
    if value.is_empty() || value.starts_with('/') || value.contains('\\') || value.contains("..") {
        return Err(AppError::BadRequest(format!("invalid file path '{path}'")));
    }
    Ok(value.to_string())
}

pub fn list_files_recursive(dir: &Path, extension: &str) -> AppResult<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let entry = entry.map_err(|error| AppError::Internal(error.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) == Some(extension) {
            files.push(path.to_path_buf());
        }
    }
    files.sort();
    Ok(files)
}
