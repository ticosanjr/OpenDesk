//! Full workspace backup and restore. Archives include workspace files and
//! native OpenCode configuration, with a manifest, SHA-256 and safe extraction.

use crate::paths::AppPaths;
use crate::{
    db::{models::CreateWorkspaceInput, workspaces},
    error::{AppError, AppResult},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::{
    fs::File,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
use uuid::Uuid;
use walkdir::WalkDir;
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

const MANIFEST: &str = "opendesk-backup.json";
const BACKUP_VERSION: u32 = 1;
const MAX_EXTRACT_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_FILES: usize = 100_000;

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
struct Manifest {
    id: String,
    workspace_id: String,
    workspace_name: String,
    created_at: String,
    files: usize,
    bytes: u64,
    version: u32,
}

fn backup_root(paths: &AppPaths) -> PathBuf {
    paths.backups_dir.join("workspaces")
}

fn workspace_dir(paths: &AppPaths, workspace_id: &str) -> AppResult<PathBuf> {
    if workspace_id.trim().is_empty()
        || workspace_id.contains('/')
        || workspace_id.contains('\\')
        || workspace_id.contains("..")
    {
        return Err(AppError::BadRequest("workspace id inválido".into()));
    }
    Ok(backup_root(paths).join(workspace_id))
}

fn sidecar_path(zip_path: &Path) -> PathBuf {
    let mut name = zip_path.as_os_str().to_os_string();
    name.push(".json");
    PathBuf::from(name)
}

fn sha256_file(path: &Path) -> AppResult<String> {
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

fn collect_files(root: &Path) -> AppResult<Vec<(PathBuf, PathBuf)>> {
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

pub async fn create(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_id: &str,
) -> AppResult<WorkspaceBackup> {
    let workspace = workspaces::get(pool, workspace_id).await?;
    let root = PathBuf::from(&workspace.root_path);
    if !root.is_dir() {
        return Err(AppError::NotFound(format!(
            "pasta do workspace não encontrada: {}",
            root.display()
        )));
    }
    let files = collect_files(&root)?;
    let bytes = files
        .iter()
        .filter_map(|(path, _)| std::fs::metadata(path).ok().map(|meta| meta.len()))
        .sum::<u64>();
    let id = Uuid::now_v7().to_string();
    let created_at = Utc::now().to_rfc3339();
    let directory = workspace_dir(paths, workspace_id)?;
    std::fs::create_dir_all(&directory)?;
    let file_name = format!("{}-{id}.zip", Utc::now().format("%Y%m%dT%H%M%SZ"));
    let zip_path = directory.join(&file_name);
    let temp_path = directory.join(format!(".{id}.tmp"));
    let manifest = Manifest {
        id: id.clone(),
        workspace_id: workspace.id.clone(),
        workspace_name: workspace.name.clone(),
        created_at: created_at.clone(),
        files: files.len(),
        bytes,
        version: BACKUP_VERSION,
    };

    let write_result = (|| -> AppResult<()> {
        let file = File::create(&temp_path)?;
        let mut zip = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        zip.start_file(MANIFEST, options)
            .map_err(|error| AppError::Internal(format!("zip manifest: {error}")))?;
        zip.write_all(&serde_json::to_vec_pretty(&manifest)?)?;
        for (path, relative) in &files {
            let name = relative.to_string_lossy().replace('\\', "/");
            zip.start_file(name, options)
                .map_err(|error| AppError::Internal(format!("zip entry: {error}")))?;
            let mut source = File::open(path)?;
            std::io::copy(&mut source, &mut zip)?;
        }
        zip.finish()
            .map_err(|error| AppError::Internal(format!("zip finish: {error}")))?;
        std::fs::rename(&temp_path, &zip_path)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&temp_path);
    }
    write_result?;

    let sha256 = sha256_file(&zip_path)?;
    let size_bytes = std::fs::metadata(&zip_path)?.len();
    let backup = WorkspaceBackup {
        id,
        workspace_id: workspace.id,
        workspace_name: workspace.name,
        file_name,
        sha256,
        size_bytes,
        files: files.len(),
        bytes,
        created_at,
        version: BACKUP_VERSION,
    };
    std::fs::write(sidecar_path(&zip_path), serde_json::to_vec_pretty(&backup)?)?;
    Ok(backup)
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

fn backup_path(
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
    created
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[tokio::test]
    async fn creates_lists_restores_and_deletes_a_workspace_backup() {
        let ctx = test_support::context().await;
        std::fs::create_dir_all(ctx.workspace_root.join(".opencode/agents")).expect("agents");
        std::fs::create_dir_all(ctx.workspace_root.join("input")).expect("input dir");
        std::fs::write(
            ctx.workspace_root.join(".opencode/agents/build.md"),
            "---\ndescription: build\n---\nbody\n",
        )
        .expect("agent");
        std::fs::write(ctx.workspace_root.join("input/brief.md"), "brief").expect("input");
        std::fs::create_dir_all(ctx.workspace_root.join("node_modules/pkg")).expect("nm");
        std::fs::write(ctx.workspace_root.join("node_modules/pkg/x.js"), "x").expect("nm file");

        let backup = create(&ctx.db, &ctx.paths, &ctx.workspace_id)
            .await
            .expect("backup");
        assert_eq!(backup.files, 2, "node_modules must be excluded");
        assert!(backup.size_bytes > 0);
        assert_eq!(list(&ctx.paths, &ctx.workspace_id).expect("list").len(), 1);

        let target = ctx.temp.path().join("restored-workspace");
        let restored = restore_as_new(
            &ctx.db,
            &ctx.paths,
            &ctx.workspace_id,
            &backup.id,
            RestoreInput {
                target_path: target.to_string_lossy().to_string(),
                name: Some("Restaurado".into()),
            },
        )
        .await
        .expect("restore");
        assert_eq!(restored.name, "Restaurado");
        assert!(target.join("input/brief.md").is_file());
        assert!(target.join(".opencode/agents/build.md").is_file());
        assert!(!target.join("node_modules").exists());

        delete(&ctx.paths, &ctx.workspace_id, &backup.id).expect("delete");
        assert!(
            list(&ctx.paths, &ctx.workspace_id)
                .expect("list")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn refuses_non_empty_target_and_corrupted_archive() {
        let ctx = test_support::context().await;
        std::fs::create_dir_all(ctx.workspace_root.join("input")).expect("input dir");
        std::fs::write(ctx.workspace_root.join("input/brief.md"), "brief").expect("input");
        let backup = create(&ctx.db, &ctx.paths, &ctx.workspace_id)
            .await
            .expect("backup");
        let target = ctx.temp.path().join("occupied");
        std::fs::create_dir_all(&target).expect("target");
        std::fs::write(target.join("keep.txt"), "keep").expect("keep");
        assert!(
            restore_as_new(
                &ctx.db,
                &ctx.paths,
                &ctx.workspace_id,
                &backup.id,
                RestoreInput {
                    target_path: target.to_string_lossy().to_string(),
                    name: None,
                },
            )
            .await
            .is_err()
        );
        let (path, _) = get(&ctx.paths, &ctx.workspace_id, &backup.id).expect("path");
        std::fs::write(&path, "not a zip").expect("corrupt");
        assert!(
            restore_as_new(
                &ctx.db,
                &ctx.paths,
                &ctx.workspace_id,
                &backup.id,
                RestoreInput {
                    target_path: ctx
                        .temp
                        .path()
                        .join("corrupt-target")
                        .to_string_lossy()
                        .to_string(),
                    name: None,
                },
            )
            .await
            .is_err()
        );
    }
}
