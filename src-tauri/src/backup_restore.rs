use crate::{
    db::{models::CreateWorkspaceInput, workspaces},
    error::{AppError, AppResult},
    paths::AppPaths,
};
use serde::Deserialize;
use sqlx::SqlitePool;
use std::{
    fs::File,
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;
use zip::ZipArchive;

use super::manifest::{MANIFEST, MAX_EXTRACT_BYTES, MAX_FILES, backup_path, sha256_file};

fn safe_entry(entry: &zip::read::ZipFile<'_, File>) -> AppResult<PathBuf> {
    if entry.is_dir() {
        return Ok(PathBuf::new());
    }
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

#[derive(Debug, Deserialize)]
pub struct RestoreInput {
    pub target_path: String,
    #[serde(default)]
    pub name: Option<String>,
}

pub async fn restore_as_new(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_id: &str,
    backup_id: &str,
    input: RestoreInput,
) -> AppResult<workspaces::Workspace> {
    let (zip_path, backup) = backup_path(paths, workspace_id, backup_id)?;
    let hash = sha256_file(&zip_path)?;
    if hash != backup.sha256 {
        return Err(AppError::Conflict(
            "backup corrompido: SHA-256 não confere".into(),
        ));
    }
    let target = PathBuf::from(input.target_path.trim());
    if target.as_os_str().is_empty() {
        return Err(AppError::BadRequest("target_path é obrigatório".into()));
    }
    let target_existed = target.exists();
    if target_existed {
        let empty = std::fs::read_dir(&target)?.next().is_none();
        if !empty {
            return Err(AppError::Conflict(
                "destino precisa estar vazio ou não existir".into(),
            ));
        }
    } else if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let parent = target
        .parent()
        .ok_or_else(|| AppError::BadRequest("target_path sem diretório pai".into()))?;
    let temp = parent.join(format!(".opendesk-restore-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&temp)?;

    let extract = (|| -> AppResult<()> {
        let file = File::open(&zip_path)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|error| AppError::Internal(format!("zip inválido: {error}")))?;
        let mut total = 0u64;
        let mut files = 0usize;
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|error| AppError::Internal(format!("zip entry: {error}")))?;
            let relative = safe_entry(&entry)?;
            if relative.as_os_str().is_empty() {
                continue;
            }
            if relative == Path::new(MANIFEST) {
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
            let destination = temp.join(&relative);
            if entry.is_dir() {
                std::fs::create_dir_all(&destination)?;
                continue;
            }
            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut output = File::create(&destination)?;
            std::io::copy(&mut entry, &mut output)?;
        }
        Ok(())
    })();
    if let Err(error) = extract {
        let _ = std::fs::remove_dir_all(&temp);
        return Err(error);
    }

    if target_existed {
        for entry in std::fs::read_dir(&temp)? {
            let entry = entry?;
            std::fs::rename(entry.path(), target.join(entry.file_name()))?;
        }
        let _ = std::fs::remove_dir_all(&temp);
    } else {
        std::fs::rename(&temp, &target)?;
    }

    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{} (restaurado)", backup.workspace_name));
    let created = workspaces::create(
        pool,
        paths,
        CreateWorkspaceInput {
            name,
            root_path: target.to_string_lossy().to_string(),
            description: Some(format!(
                "Restaurado do backup {} de {}",
                backup.id, backup.workspace_name
            )),
            color_hex: None,
            default_router_policy: None,
            default_email_recipient: None,
            google_drive_folder_id: None,
            timezone: None,
        },
    )
    .await;
    if created.is_err() && !target_existed {
        let _ = std::fs::remove_dir_all(&target);
    }
    created.map_err(Into::into)
}
