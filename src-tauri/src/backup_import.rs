//! Imports a `.zip` produced by OpenDesk into the local backup catalog after a
//! full dry-run validation (paths, per-file SHA-256 and state snapshot hash).

use crate::{
    error::{AppError, AppResult},
    paths::AppPaths,
};
use chrono::Utc;
use std::fs::File;
use uuid::Uuid;
use zip::ZipArchive;

use super::extract::{extract_files, read_bounded, read_manifest};
use super::manifest::{
    BACKUP_VERSION, MAX_EXTRACT_BYTES, MAX_IMPORT_BYTES, WorkspaceBackup, sha256_file,
    sidecar_path, workspace_dir,
};
use super::state::{self, LEGACY_STATE_ENTRY, STATE_ENTRY};

fn clean_name(value: &str) -> String {
    let name = value
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("backup.zip")
        .chars()
        .filter(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
        .collect::<String>();
    let name = name.trim_matches('.');
    if name.is_empty() {
        "backup.zip".into()
    } else if name.ends_with(".zip") {
        name.into()
    } else {
        format!("{name}.zip")
    }
}

pub async fn import_archive(
    paths: &AppPaths,
    workspace_id: &str,
    file_name: &str,
    bytes: &[u8],
) -> AppResult<WorkspaceBackup> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_IMPORT_BYTES {
        return Err(AppError::BadRequest(
            "arquivo de backup vazio ou acima de 256 MB".into(),
        ));
    }
    let directory = workspace_dir(paths, workspace_id)?;
    std::fs::create_dir_all(&directory)?;
    let id = Uuid::now_v7().to_string();
    let final_name = format!(
        "imported-{}-{id}-{}",
        Utc::now().format("%Y%m%dT%H%M%SZ"),
        clean_name(file_name)
    );
    let temp = directory.join(format!(".{id}.import"));
    std::fs::write(&temp, bytes)?;
    let final_path = directory.join(&final_name);
    let result = (|| -> AppResult<WorkspaceBackup> {
        let file = File::open(&temp)?;
        let mut archive = ZipArchive::new(file)
            .map_err(|error| AppError::BadRequest(format!("zip inválido: {error}")))?;
        let manifest = read_manifest(&mut archive)?;
        if manifest.version == 0 || manifest.version > BACKUP_VERSION {
            return Err(AppError::BadRequest(
                "versão de backup não suportada".into(),
            ));
        }
        let staging = directory.join(format!(".validate-{id}"));
        std::fs::create_dir_all(&staging)?;
        let validation = (|| -> AppResult<()> {
            extract_files(&mut archive, &staging, &manifest)?;
            let state_bytes = match read_bounded(&mut archive, STATE_ENTRY, MAX_EXTRACT_BYTES)? {
                Some(bytes) => Some(bytes),
                None => read_bounded(&mut archive, LEGACY_STATE_ENTRY, MAX_EXTRACT_BYTES)?,
            };
            if manifest.version >= 2 && manifest.includes.sqlite_state {
                let bytes = state_bytes.as_deref().ok_or_else(|| {
                    AppError::BadRequest("backup declara estado SQLite, mas não o contém".into())
                })?;
                let expected = manifest
                    .state_sha256
                    .as_deref()
                    .or(manifest.db_sha256.as_deref())
                    .ok_or_else(|| {
                        AppError::BadRequest("snapshot sem SHA-256 no manifesto".into())
                    })?;
                if state::sha256_bytes(bytes) != expected {
                    return Err(AppError::Conflict(
                        "snapshot SQLite não confere com o manifesto".into(),
                    ));
                }
                let _: state::WorkspaceState = serde_json::from_slice(bytes)
                    .map_err(|error| AppError::BadRequest(format!("snapshot inválido: {error}")))?;
            }
            Ok(())
        })();
        let _ = std::fs::remove_dir_all(&staging);
        validation?;
        std::fs::rename(&temp, &final_path)?;
        let sha256 = sha256_file(&final_path)?;
        let size_bytes = std::fs::metadata(&final_path)?.len();
        let backup = WorkspaceBackup {
            id,
            workspace_id: workspace_id.to_string(),
            workspace_name: manifest.workspace_name,
            file_name: final_name.clone(),
            sha256,
            size_bytes,
            files: manifest.files,
            bytes: manifest.bytes,
            created_at: Utc::now().to_rfc3339(),
            version: manifest.version,
            includes: manifest.includes,
            counts: manifest.counts,
        };
        std::fs::write(
            sidecar_path(&final_path),
            serde_json::to_vec_pretty(&backup)?,
        )?;
        Ok(backup)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
        let _ = std::fs::remove_file(&final_path);
        let _ = std::fs::remove_file(sidecar_path(&final_path));
    }
    result
}
