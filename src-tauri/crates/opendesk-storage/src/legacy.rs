//! V1 detection, export and one-shot migration.
//!
//! The V2 baseline is a clean break. A V1 database is never opened by the V2
//! schema: the app detects it before initializing, exports workspaces/recipes/
//! settings to JSON/YAML and renames the old file. Only references are
//! exported; secret values never leave the local `SecretStore`.

use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use crate::paths::AppPaths;

#[derive(Debug, Clone, Serialize)]
pub struct ExportSummary {
    pub schema_version: i64,
    pub workspaces: usize,
    pub recipes: usize,
    pub settings: usize,
    pub out: PathBuf,
}

#[derive(Debug, Serialize)]
struct ExportBundle {
    schema_version: i64,
    exported_at: String,
    workspaces: Vec<WorkspaceExport>,
    recipes: Vec<RecipeExport>,
    settings: Vec<SettingExport>,
}

#[derive(Debug, Serialize)]
struct WorkspaceExport {
    id: String,
    name: String,
    root_path: String,
}

#[derive(Debug, Serialize)]
struct RecipeExport {
    id: String,
    workspace_id: String,
    name: String,
    file_path: String,
    trigger_type: String,
    is_enabled: bool,
    yaml: Option<String>,
}

#[derive(Debug, Serialize)]
struct SettingExport {
    key: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct Manifest {
    schema_version: i64,
    exported_at: String,
    format: String,
    counts: Counts,
    files: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
struct Counts {
    workspaces: usize,
    recipes: usize,
    settings: usize,
}

/// `Some(version)` when the file is a V1 database, `None` for a V2 database,
/// a fresh file or a non-OpenDesk file.
pub async fn legacy_version(path: &Path) -> Result<Option<i64>> {
    if !path.is_file() {
        return Ok(None);
    }
    let pool = open_read_only(path).await?;
    let version = v1_version_on(&pool).await?;
    pool.close().await;
    Ok(version)
}

pub async fn export_v1(db: &Path, out: &Path, yaml: bool) -> Result<ExportSummary> {
    if !db.is_file() {
        bail!("banco não encontrado: {}", db.display());
    }
    if out.exists() {
        let non_empty = fs::read_dir(out)
            .with_context(|| format!("não foi possível ler {}", out.display()))?
            .next()
            .is_some();
        if non_empty {
            bail!("diretório de saída não vazio: {}", out.display());
        }
    }
    let pool = open_read_only(db).await?;
    let schema_version = v1_version_on(&pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("este banco não é V1; não há nada a exportar"))?;

    let workspaces = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, name, root_path FROM workspaces ORDER BY name, id",
    )
    .fetch_all(&pool)
    .await
    .context("workspaces")?
    .into_iter()
    .map(|(id, name, root_path)| WorkspaceExport {
        id,
        name,
        root_path,
    })
    .collect::<Vec<_>>();

    let recipes = sqlx::query_as::<_, (String, String, String, String, String, i64)>(
        "SELECT id, workspace_id, name, COALESCE(file_path, ''), COALESCE(trigger_type, ''),
                CAST(COALESCE(is_enabled, 1) AS INTEGER)
         FROM recipes_index ORDER BY workspace_id, name, id",
    )
    .fetch_all(&pool)
    .await
    .context("recipes")?
    .into_iter()
    .map(
        |(id, workspace_id, name, file_path, trigger_type, is_enabled)| {
            let yaml = read_recipe_yaml(&file_path);
            RecipeExport {
                id,
                workspace_id,
                name,
                file_path,
                trigger_type,
                is_enabled: is_enabled != 0,
                yaml,
            }
        },
    )
    .collect::<Vec<_>>();

    let settings = sqlx::query_as::<_, (String, String)>(
        "SELECT key, value_json AS value FROM app_settings ORDER BY key",
    )
    .fetch_all(&pool)
    .await
    .context("settings")?
    .into_iter()
    .map(|(key, value)| SettingExport { key, value })
    .collect::<Vec<_>>();
    pool.close().await;

    let exported_at = Utc::now().to_rfc3339();
    let bundle = ExportBundle {
        schema_version,
        exported_at: exported_at.clone(),
        workspaces,
        recipes,
        settings,
    };
    fs::create_dir_all(out).with_context(|| format!("não foi possível criar {}", out.display()))?;

    let mut files = BTreeMap::new();
    let (bundle_name, bundle_bytes) = if yaml {
        (
            "export.yaml",
            serde_yaml::to_string(&bundle)
                .context("serializar YAML")?
                .into_bytes(),
        )
    } else {
        (
            "export.json",
            serde_json::to_vec_pretty(&bundle).context("serializar JSON")?,
        )
    };
    write_hashed(out, bundle_name, &bundle_bytes, &mut files)?;

    let recipes_dir = out.join("recipes");
    for recipe in &bundle.recipes {
        if let Some(yaml) = &recipe.yaml {
            fs::create_dir_all(&recipes_dir)
                .with_context(|| format!("não foi possível criar {}", recipes_dir.display()))?;
            let name = format!("{}.yaml", safe_name(&recipe.id));
            write_hashed(&recipes_dir, &name, yaml.as_bytes(), &mut files)?;
        }
    }

    let summary = ExportSummary {
        schema_version,
        workspaces: bundle.workspaces.len(),
        recipes: bundle.recipes.len(),
        settings: bundle.settings.len(),
        out: out.to_path_buf(),
    };
    let manifest = Manifest {
        schema_version,
        exported_at,
        format: if yaml { "yaml" } else { "json" }.into(),
        counts: Counts {
            workspaces: summary.workspaces,
            recipes: summary.recipes,
            settings: summary.settings,
        },
        files,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).context("serializar manifesto")?;
    fs::write(out.join("manifest.json"), manifest_bytes).with_context(|| {
        format!(
            "não foi possível escrever {}",
            out.join("manifest.json").display()
        )
    })?;
    Ok(summary)
}

/// Exports the V1 database and moves it aside so the next `init` creates a
/// fresh V2 database. The old file is preserved as `opendesk.db.v1-<timestamp>`
/// together with its WAL sidecars.
pub async fn migrate_v1(paths: &AppPaths, out: &Path) -> Result<ExportSummary> {
    let Some(_version) = legacy_version(&paths.db_path).await? else {
        bail!(
            "o banco {} não é V1; nada a migrar",
            paths.db_path.display()
        );
    };
    let summary = export_v1(&paths.db_path, out, false).await?;
    let stamp = Utc::now().format("%Y%m%d-%H%M%S");
    let mut backup = paths
        .db_path
        .with_file_name(format!("opendesk.db.v1-{stamp}"));
    let mut counter = 1;
    while backup.exists() {
        backup = paths
            .db_path
            .with_file_name(format!("opendesk.db.v1-{stamp}-{counter}"));
        counter += 1;
    }
    fs::rename(&paths.db_path, &backup).with_context(|| {
        format!(
            "não foi possível renomear {} para {}",
            paths.db_path.display(),
            backup.display()
        )
    })?;
    for suffix in ["-wal", "-shm"] {
        let from = PathBuf::from(format!("{}{}", paths.db_path.display(), suffix));
        if from.exists() {
            let to = PathBuf::from(format!("{}{}", backup.display(), suffix));
            if let Err(error) = fs::rename(&from, &to) {
                tracing::warn!(error = %error, path = %from.display(), "could not move the V1 sidecar");
            }
        }
    }
    tracing::info!(
        backup = %backup.display(),
        export = %summary.out.display(),
        "V1 database migrated and preserved"
    );
    Ok(summary)
}

async fn open_read_only(path: &Path) -> Result<SqlitePool> {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .read_only(true)
                .pragma("query_only", "true"),
        )
        .await
        .with_context(|| format!("não foi possível abrir {}", path.display()))
}

async fn v1_version_on(pool: &SqlitePool) -> Result<Option<i64>> {
    let has_migrations: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await
    .context("_sqlx_migrations")?;
    if has_migrations == 0 {
        return Ok(None);
    }
    let is_v2 =
        sqlx::query_scalar::<_, String>("SELECT value FROM v2_meta WHERE key = 'schema_version'")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    if is_v2.is_some() {
        return Ok(None);
    }
    let version: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
        .context("_sqlx_migrations version")?;
    Ok((version > 0).then_some(version))
}

fn read_recipe_yaml(file_path: &str) -> Option<String> {
    let path = Path::new(file_path);
    if file_path.trim().is_empty() {
        return None;
    }
    let metadata = fs::metadata(path).ok()?;
    if metadata.len() > 2 * 1024 * 1024 {
        return None;
    }
    fs::read_to_string(path).ok()
}

fn write_hashed(
    dir: &Path,
    name: &str,
    bytes: &[u8],
    files: &mut BTreeMap<String, String>,
) -> Result<()> {
    let path = dir.join(name);
    fs::write(&path, bytes)
        .with_context(|| format!("não foi possível escrever {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    files.insert(name.to_string(), digest);
    Ok(())
}

fn safe_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "legacy_tests.rs"]
mod legacy_tests;
