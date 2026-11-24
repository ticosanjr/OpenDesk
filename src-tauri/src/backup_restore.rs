use crate::{
    db::{models::CreateWorkspaceInput, workspaces},
    error::{AppError, AppResult},
    paths::AppPaths,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{
    fs::File,
    path::{Path, PathBuf},
};
use uuid::Uuid;
use zip::ZipArchive;

use super::extract::{extract_files, extract_run_logs, read_bounded, read_manifest};
use super::manifest::{BACKUP_VERSION, MAX_EXTRACT_BYTES, backup_path, sha256_file};
use super::portable as secrets;
use super::state::{self, SECRETS_ENTRY, STATE_ENTRY, WorkspaceState};

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RestoreMode {
    #[default]
    NewWorkspace,
    InPlace,
}

#[derive(Debug, Default, Deserialize)]
pub struct RestoreInput {
    #[serde(default)]
    pub target_path: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub mode: RestoreMode,
    #[serde(default)]
    pub passphrase: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RestoreReport {
    pub workspace_id: String,
    pub mode: RestoreMode,
    pub counts: std::collections::BTreeMap<String, u64>,
    pub missing_secrets: Vec<String>,
    pub pre_restore_backup: Option<String>,
}

async fn install_state(
    connection: &mut sqlx::SqliteConnection,
    state: &WorkspaceState,
    workspace_id: &str,
    replace: bool,
) -> AppResult<()> {
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *connection)
        .await?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *connection)
        .await?;
    let result = async {
        if replace {
            clear_workspace(connection, state, workspace_id).await?;
        }
        state::restore(connection, state, workspace_id).await?;
        let violations = sqlx::query("PRAGMA foreign_key_check")
            .fetch_all(&mut *connection)
            .await?;
        if !violations.is_empty() {
            return Err(AppError::Conflict(
                "snapshot restaurado viola chaves estrangeiras".into(),
            ));
        }
        sqlx::query("COMMIT").execute(&mut *connection).await?;
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
    }
    let _ = sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&mut *connection)
        .await;
    result
}

async fn clear_workspace(
    connection: &mut sqlx::SqliteConnection,
    state: &WorkspaceState,
    workspace_id: &str,
) -> AppResult<()> {
    for table in state.tables.iter().rev() {
        if table.name == "workspaces" {
            continue;
        }
        let sql = format!(
            "DELETE FROM \"{}\" WHERE workspace_id = ?",
            table.name.replace('"', "\"\"")
        );
        sqlx::query(&sql)
            .bind(workspace_id)
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}

fn move_into(source: &Path, target: &Path) -> AppResult<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let destination = target.join(entry.file_name());
        if destination.exists() {
            if destination.is_dir() {
                let _ = std::fs::remove_dir_all(&destination);
            } else {
                let _ = std::fs::remove_file(&destination);
            }
        }
        std::fs::rename(entry.path(), destination)?;
    }
    Ok(())
}

pub async fn restore_as_new(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_id: &str,
    backup_id: &str,
    input: RestoreInput,
) -> AppResult<workspaces::Workspace> {
    let input = RestoreInput {
        mode: RestoreMode::NewWorkspace,
        ..input
    };
    restore(pool, paths, workspace_id, backup_id, input)
        .await
        .map(|(workspace, _)| workspace)
}

pub async fn restore(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_id: &str,
    backup_id: &str,
    input: RestoreInput,
) -> AppResult<(workspaces::Workspace, RestoreReport)> {
    let (zip_path, backup) = backup_path(paths, workspace_id, backup_id)?;
    let hash = sha256_file(&zip_path)?;
    if hash != backup.sha256 {
        return Err(AppError::Conflict(
            "backup corrompido: SHA-256 não confere".into(),
        ));
    }
    let file = File::open(&zip_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| AppError::Internal(format!("zip inválido: {error}")))?;
    let manifest = read_manifest(&mut archive)?;
    if manifest.version > BACKUP_VERSION {
        return Err(AppError::BadRequest(
            "backup de versão futura; atualize o OpenDesk".into(),
        ));
    }
    let target = PathBuf::from(input.target_path.trim());
    if input.mode == RestoreMode::NewWorkspace {
        if target.as_os_str().is_empty() {
            return Err(AppError::BadRequest("target_path é obrigatório".into()));
        }
        if target.exists() {
            let empty = std::fs::read_dir(&target)?.next().is_none();
            if !empty {
                return Err(AppError::Conflict(
                    "destino precisa estar vazio ou não existir".into(),
                ));
            }
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let in_place_workspace = if input.mode == RestoreMode::InPlace {
        Some(workspaces::get(pool, workspace_id).await?)
    } else {
        None
    };
    let in_place_root = in_place_workspace
        .as_ref()
        .map(|workspace| PathBuf::from(&workspace.root_path));
    let parent = if input.mode == RestoreMode::NewWorkspace {
        target
            .parent()
            .ok_or_else(|| AppError::BadRequest("target_path sem diretório pai".into()))?
    } else {
        in_place_root
            .as_deref()
            .and_then(Path::parent)
            .ok_or_else(|| AppError::BadRequest("workspace sem diretório pai".into()))?
    };
    let staging = parent.join(format!(".opendesk-restore-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&staging)?;

    let prepared = (|| -> AppResult<(Option<WorkspaceState>, Option<secrets::PortableSecrets>)> {
        extract_files(&mut archive, &staging, &manifest)?;
        let state = match read_bounded(&mut archive, STATE_ENTRY, MAX_EXTRACT_BYTES)? {
            Some(bytes) => {
                if let Some(expected) = manifest.db_sha256.as_deref() {
                    let actual = state::sha256_bytes(&bytes);
                    if actual != expected {
                        return Err(AppError::Conflict(
                            "snapshot SQLite não confere com o manifesto".into(),
                        ));
                    }
                }
                Some(
                    serde_json::from_slice::<WorkspaceState>(&bytes).map_err(|error| {
                        AppError::BadRequest(format!("snapshot inválido: {error}"))
                    })?,
                )
            }
            None => None,
        };
        let portable = match read_bounded(&mut archive, SECRETS_ENTRY, MAX_EXTRACT_BYTES)? {
            Some(bytes) => {
                let passphrase = input.passphrase.as_deref().unwrap_or_default();
                if passphrase.trim().is_empty() {
                    return Err(AppError::BadRequest(
                        "backup contém segredos; informe a senha".into(),
                    ));
                }
                Some(secrets::decrypt(&bytes, passphrase)?)
            }
            None => None,
        };
        extract_run_logs(&mut archive, paths, &manifest)?;
        Ok((state, portable))
    })();
    let (mut state, portable) = match prepared {
        Ok(value) => value,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(error);
        }
    };

    let workspace = if input.mode == RestoreMode::InPlace {
        in_place_workspace
            .clone()
            .ok_or_else(|| AppError::NotFound(format!("workspace {workspace_id}")))?
    } else {
        let staged_target = target.clone();
        if staged_target.exists() {
            move_into(&staging, &staged_target)?;
            let _ = std::fs::remove_dir_all(&staging);
        } else {
            std::fs::rename(&staging, &staged_target)?;
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
                root_path: staged_target.to_string_lossy().to_string(),
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
        match created {
            Ok(workspace) => workspace,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&target);
                return Err(error.into());
            }
        }
    };

    let pre_restore_backup = if input.mode == RestoreMode::InPlace {
        Some(super::create(pool, paths, &workspace.id).await?.id)
    } else {
        None
    };

    let mut in_place_old: Option<PathBuf> = None;
    if input.mode == RestoreMode::InPlace {
        let root = PathBuf::from(&workspace.root_path);
        let old = root.with_file_name(format!(
            ".{}-pre-restore-{}",
            root.file_name().unwrap_or_default().to_string_lossy(),
            Utc::now().format("%Y%m%d%H%M%S")
        ));
        if root.exists() {
            std::fs::rename(&root, &old)?;
        }
        if let Err(error) = std::fs::rename(&staging, &root) {
            if old.exists() {
                let _ = std::fs::rename(&old, &root);
            }
            return Err(AppError::Io(error));
        }
        in_place_old = Some(old);
    }

    let install = async {
        if let Some(state) = state.as_mut() {
            if input.mode == RestoreMode::NewWorkspace {
                state::remap_ids(state, &workspace.id);
                state::remap_paths(state, &manifest.workspace_root, &workspace.root_path);
            }
            let mut connection = pool.acquire().await?;
            install_state(
                &mut connection,
                state,
                &workspace.id,
                input.mode == RestoreMode::InPlace,
            )
            .await?;
            if let Some(portable) = &portable {
                secrets::apply(pool, paths, &workspace.id, portable).await?;
            }
        }
        Ok::<(), AppError>(())
    }
    .await;
    if let Err(error) = install {
        if input.mode == RestoreMode::NewWorkspace {
            let _ = workspaces::delete(pool, &workspace.id).await;
            let _ = std::fs::remove_dir_all(&target);
        } else if let Some(old) = &in_place_old {
            let root = PathBuf::from(&workspace.root_path);
            let _ = std::fs::remove_dir_all(&root);
            if old.exists() {
                let _ = std::fs::rename(old, &root);
            }
        }
        return Err(error);
    }
    if let Some(old) = in_place_old {
        let _ = std::fs::remove_dir_all(&old);
    }

    let missing_secrets = if portable.is_some() {
        Vec::new()
    } else {
        state
            .as_ref()
            .and_then(|state| {
                state
                    .tables
                    .iter()
                    .find(|table| table.name == "vault_entries")
            })
            .map(|table| {
                let index = table.columns.iter().position(|column| column == "id");
                table
                    .rows
                    .iter()
                    .filter_map(|row| {
                        index
                            .and_then(|index| row.get(index))
                            .and_then(|value| value.as_str())
                            .map(str::to_string)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    };
    let report = RestoreReport {
        workspace_id: workspace.id.clone(),
        mode: input.mode,
        counts: state
            .as_ref()
            .map(WorkspaceState::counts)
            .unwrap_or_default(),
        missing_secrets,
        pre_restore_backup,
    };
    Ok((workspace, report))
}
