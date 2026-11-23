use crate::{
    db::workspaces,
    error::{AppError, AppResult},
    paths::AppPaths,
};
use chrono::Utc;
use sqlx::SqlitePool;
use std::{fs::File, io::Write, path::PathBuf};
use uuid::Uuid;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use super::manifest::{
    BACKUP_VERSION, MANIFEST, Manifest, WorkspaceBackup, collect_files, sha256_file, sidecar_path,
    workspace_dir,
};

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
