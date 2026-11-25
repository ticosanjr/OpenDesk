use crate::{
    db::workspaces,
    error::{AppError, AppResult},
    paths::AppPaths,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{collections::BTreeMap, fs::File, io::Write, path::PathBuf};
use uuid::Uuid;
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use super::manifest::{
    BACKUP_VERSION, BackupIncludes, MANIFEST, Manifest, WorkspaceBackup, collect_files,
    sha256_file, sidecar_path, workspace_dir,
};
use super::portable as backup_secrets;
use super::state::{self as backup_state, SECRETS_ENTRY, STATE_ENTRY};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct BackupOptions {
    #[serde(default)]
    pub include_logs: Option<bool>,
    #[serde(default)]
    pub passphrase: Option<String>,
}

pub async fn create(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_id: &str,
) -> AppResult<WorkspaceBackup> {
    create_with_options(pool, paths, workspace_id, BackupOptions::default()).await
}

pub async fn create_with_options(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_id: &str,
    options: BackupOptions,
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
    let mut files_sha256 = BTreeMap::new();
    for (path, relative) in &files {
        files_sha256.insert(
            relative.to_string_lossy().replace('\\', "/"),
            sha256_file(path)?,
        );
    }

    let state = backup_state::export(pool, workspace_id).await?;
    let state_bytes = serde_json::to_vec(&state)
        .map_err(|error| AppError::Internal(format!("snapshot json: {error}")))?;
    let db_sha256 = backup_state::sha256_bytes(&state_bytes);
    let run_ids = state
        .tables
        .iter()
        .find(|table| table.name == "task_runs")
        .and_then(|table| {
            let index = table.columns.iter().position(|column| column == "id")?;
            Some(
                table
                    .rows
                    .iter()
                    .filter_map(|row| row.get(index).and_then(|value| value.as_str()))
                    .map(str::to_string)
                    .collect::<Vec<_>>(),
            )
        })
        .unwrap_or_default();
    let include_logs = options.include_logs.unwrap_or(true);
    let logs = if include_logs {
        backup_state::run_log_files(paths, &run_ids)
    } else {
        Vec::new()
    };
    let secrets = match options.passphrase.as_deref() {
        Some(passphrase) if !passphrase.trim().is_empty() => {
            let portable = backup_secrets::collect(pool, paths, workspace_id).await?;
            Some(backup_secrets::encrypt(&portable, passphrase)?)
        }
        _ => None,
    };

    let id = Uuid::now_v7().to_string();
    let created_at = Utc::now().to_rfc3339();
    let directory = workspace_dir(paths, workspace_id)?;
    std::fs::create_dir_all(&directory)?;
    let file_name = format!("{}-{id}.zip", Utc::now().format("%Y%m%dT%H%M%SZ"));
    let zip_path = directory.join(&file_name);
    let temp_path = directory.join(format!(".{id}.tmp"));
    let mut counts = state.counts();
    counts.insert("files".into(), files.len() as u64);
    counts.insert("run_logs".into(), logs.len() as u64);
    let includes = BackupIncludes {
        workspace_files: true,
        sqlite_state: true,
        run_logs: include_logs,
        encrypted_secrets: secrets.is_some(),
    };
    let manifest = Manifest {
        id: id.clone(),
        workspace_id: workspace.id.clone(),
        workspace_name: workspace.name.clone(),
        created_at: created_at.clone(),
        files: files.len(),
        bytes,
        version: BACKUP_VERSION,
        includes: includes.clone(),
        counts: counts.clone(),
        state_sha256: Some(db_sha256.clone()),
        db_sha256: Some(db_sha256),
        files_sha256,
        workspace_root: workspace.root_path.clone(),
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
        zip.start_file(STATE_ENTRY, options)
            .map_err(|error| AppError::Internal(format!("zip state: {error}")))?;
        zip.write_all(&state_bytes)?;
        for (name, bytes) in &logs {
            zip.start_file(name, options)
                .map_err(|error| AppError::Internal(format!("zip run log: {error}")))?;
            zip.write_all(bytes)?;
        }
        if let Some(bytes) = &secrets {
            zip.start_file(SECRETS_ENTRY, options)
                .map_err(|error| AppError::Internal(format!("zip secrets: {error}")))?;
            zip.write_all(bytes)?;
        }
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
        includes,
        counts,
    };
    std::fs::write(sidecar_path(&zip_path), serde_json::to_vec_pretty(&backup)?)?;
    Ok(backup)
}
