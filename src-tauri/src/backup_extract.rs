use crate::{
    error::{AppError, AppResult},
    paths::AppPaths,
};
use std::{
    fs::File,
    io::Read,
    path::{Component, Path, PathBuf},
};
use zip::ZipArchive;

use super::manifest::{MANIFEST, MAX_EXTRACT_BYTES, MAX_FILES, Manifest, sha256_file};
use super::state::{self, SECRETS_ENTRY, STATE_ENTRY};

pub(super) fn safe_entry(entry: &zip::read::ZipFile<'_, File>) -> AppResult<PathBuf> {
    if let Some(mode) = entry.unix_mode()
        && mode & 0o170000 == 0o120000
    {
        return Err(AppError::BadRequest(
            "backup contém symlink; restauração recusada".into(),
        ));
    }
    let enclosed = entry
        .enclosed_name()
        .ok_or_else(|| AppError::BadRequest("backup contém caminho inseguro".into()))?;
    if enclosed
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(AppError::BadRequest(
            "backup contém caminho inseguro".into(),
        ));
    }
    Ok(enclosed)
}

pub(super) fn read_manifest(archive: &mut ZipArchive<File>) -> AppResult<Manifest> {
    let mut entry = archive
        .by_name(MANIFEST)
        .map_err(|_| AppError::BadRequest("backup sem manifesto".into()))?;
    let mut bytes = Vec::new();
    (&mut entry).take(1024 * 1024).read_to_end(&mut bytes)?;
    serde_json::from_slice(&bytes)
        .map_err(|error| AppError::BadRequest(format!("manifesto inválido: {error}")))
}

pub(super) fn read_bounded(
    archive: &mut ZipArchive<File>,
    name: &str,
    limit: u64,
) -> AppResult<Option<Vec<u8>>> {
    let mut entry = match archive.by_name(name) {
        Ok(entry) => entry,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(error) => return Err(AppError::Internal(format!("zip entry: {error}"))),
    };
    if entry.size() > limit {
        return Err(AppError::BadRequest(format!(
            "entrada {name} excede o limite"
        )));
    }
    let mut bytes = Vec::new();
    (&mut entry).take(limit + 1).read_to_end(&mut bytes)?;
    Ok(Some(bytes))
}

pub(super) fn extract_files(
    archive: &mut ZipArchive<File>,
    destination: &Path,
    manifest: &Manifest,
) -> AppResult<()> {
    let mut total = 0u64;
    let mut files = 0usize;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| AppError::Internal(format!("zip entry: {error}")))?;
        let relative = safe_entry(&entry)?;
        if relative.as_os_str().is_empty() || relative == Path::new(MANIFEST) {
            continue;
        }
        let name = relative.to_string_lossy().replace('\\', "/");
        if name == STATE_ENTRY || name == SECRETS_ENTRY || name.starts_with(state::RUN_LOGS_PREFIX)
        {
            continue;
        }
        files += 1;
        if files > MAX_FILES {
            return Err(AppError::BadRequest("backup com arquivos demais".into()));
        }
        total += entry.size();
        if total > MAX_EXTRACT_BYTES {
            return Err(AppError::BadRequest("backup excede 2 GiB".into()));
        }
        let target = destination.join(&relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&target)?;
        std::io::copy(&mut entry, &mut output)?;
        if manifest.version >= 2
            && let Some(expected) = manifest.files_sha256.get(&name)
        {
            let actual = sha256_file(&target)?;
            if &actual != expected {
                return Err(AppError::Conflict(format!(
                    "arquivo {name} não confere com o manifesto"
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn extract_run_logs(
    archive: &mut ZipArchive<File>,
    paths: &AppPaths,
    manifest: &Manifest,
) -> AppResult<()> {
    if !manifest.includes.run_logs {
        return Ok(());
    }
    std::fs::create_dir_all(&paths.run_logs_dir)?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| AppError::Internal(format!("zip entry: {error}")))?;
        let name = entry
            .name()
            .map_err(|error| AppError::Internal(format!("zip entry: {error}")))?
            .to_string();
        let Some(file_name) = name.strip_prefix(state::RUN_LOGS_PREFIX) else {
            continue;
        };
        if file_name.contains('/') || file_name.contains("..") {
            return Err(AppError::BadRequest("run log com caminho inseguro".into()));
        }
        let mut bytes = Vec::new();
        (&mut entry)
            .take(64 * 1024 * 1024)
            .read_to_end(&mut bytes)?;
        let path = paths.run_logs_dir.join(file_name);
        std::fs::write(path, bytes)?;
    }
    Ok(())
}
