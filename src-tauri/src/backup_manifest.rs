use crate::{
    error::{AppError, AppResult},
    paths::AppPaths,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    path::{Component, Path, PathBuf},
};
use walkdir::WalkDir;

pub(super) const MANIFEST: &str = "opendesk-backup.json";
pub(super) const BACKUP_VERSION: u32 = 1;
pub(super) const MAX_EXTRACT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
pub(super) const MAX_FILES: usize = 100_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceBackup {
    pub id: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub file_name: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub files: usize,
    pub bytes: u64,
    pub created_at: String,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Manifest {
    pub id: String,
    pub workspace_id: String,
    pub workspace_name: String,
    pub created_at: String,
    pub files: usize,
    pub bytes: u64,
    pub version: u32,
}

pub(super) fn backup_root(paths: &AppPaths) -> PathBuf {
    paths.backups_dir.join("workspaces")
}

pub(super) fn workspace_dir(paths: &AppPaths, workspace_id: &str) -> AppResult<PathBuf> {
    if workspace_id.trim().is_empty()
        || workspace_id.contains('/')
        || workspace_id.contains('\\')
        || workspace_id.contains("..")
    {
        return Err(AppError::BadRequest("workspace id inválido".into()));
    }
    Ok(backup_root(paths).join(workspace_id))
}

pub(super) fn sidecar_path(zip_path: &Path) -> PathBuf {
    let mut name = zip_path.as_os_str().to_os_string();
    name.push(".json");
    PathBuf::from(name)
}

pub(super) fn sha256_file(path: &Path) -> AppResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn excluded(relative: &Path) -> bool {
    const EXCLUDED: [&str; 13] = [
        ".git",
        "node_modules",
        "target",
        "dist",
        "build",
        ".next",
        ".turbo",
        ".venv",
        "venv",
        "__pycache__",
        ".cache",
        ".DS_Store",
        "Thumbs.db",
    ];
    relative.components().any(|component| {
        let Component::Normal(name) = component else {
            return false;
        };
        let name = name.to_string_lossy();
        EXCLUDED.iter().any(|excluded| name == *excluded)
    })
}

pub(super) fn collect_files(root: &Path) -> AppResult<Vec<(PathBuf, PathBuf)>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry.map_err(|error| AppError::Internal(error.to_string()))?;
        let path = entry.path();
        if path == root {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|error| AppError::Internal(error.to_string()))?
            .to_path_buf();
        if excluded(&relative) {
            continue;
        }
        let file_type = entry.file_type();
        if file_type.is_symlink() || !file_type.is_file() {
            continue;
        }
        files.push((path.to_path_buf(), relative));
        if files.len() > MAX_FILES {
            return Err(AppError::BadRequest(format!(
                "workspace tem mais de {MAX_FILES} arquivos; reduza antes de fazer backup"
            )));
        }
    }
    files.sort_by(|left, right| left.1.cmp(&right.1));
    Ok(files)
}

pub(super) fn backup_path(
    paths: &AppPaths,
    workspace_id: &str,
    id: &str,
) -> AppResult<(PathBuf, WorkspaceBackup)> {
    if id.trim().is_empty() || id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err(AppError::BadRequest("backup id inválido".into()));
    }
    let backup = list(paths, workspace_id)?
        .into_iter()
        .find(|backup| backup.id == id)
        .ok_or_else(|| AppError::NotFound(format!("backup {id}")))?;
    let path = workspace_dir(paths, workspace_id)?.join(&backup.file_name);
    if !path.is_file() {
        return Err(AppError::NotFound(format!("arquivo do backup {id}")));
    }
    Ok((path, backup))
}

pub fn list(paths: &AppPaths, workspace_id: &str) -> AppResult<Vec<WorkspaceBackup>> {
    let directory = workspace_dir(paths, workspace_id)?;
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut backups = Vec::new();
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".zip.json") {
            continue;
        }
        let Ok(raw) = std::fs::read(entry.path()) else {
            continue;
        };
        let Ok(backup) = serde_json::from_slice::<WorkspaceBackup>(&raw) else {
            continue;
        };
        if backup.workspace_id == workspace_id {
            backups.push(backup);
        }
    }
    backups.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(backups)
}

pub fn get(
    paths: &AppPaths,
    workspace_id: &str,
    id: &str,
) -> AppResult<(PathBuf, WorkspaceBackup)> {
    backup_path(paths, workspace_id, id)
}

pub fn delete(paths: &AppPaths, workspace_id: &str, id: &str) -> AppResult<()> {
    let (path, _) = backup_path(paths, workspace_id, id)?;
    std::fs::remove_file(&path)?;
    let _ = std::fs::remove_file(sidecar_path(&path));
    Ok(())
}
