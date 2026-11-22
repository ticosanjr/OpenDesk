//! `opendesk-cli migrate-v1-export`: archival bridge out of a V1 database.
//!
//! The V2 baseline is a clean break, so this command reads a V1 SQLite file in
//! read-only mode and writes workspaces, recipes and settings as JSON/YAML plus
//! a manifest with checksums. It never exports secret values: only the shape
//! needed to reconfigure integrations in V2.

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

const HELP: &str =
    "uso: opendesk-cli migrate-v1-export --db <v1.db> --out <dir> [--format json|yaml]";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Json,
    Yaml,
}

#[derive(Debug)]
struct Options {
    db: PathBuf,
    out: PathBuf,
    format: Format,
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
    /// Raw YAML when the file is still readable and small enough.
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

pub fn run(args: &[String]) -> Result<(), String> {
    let options = parse(args)?;
    let runtime = tokio::runtime::Runtime::new().map_err(|error| error.to_string())?;
    runtime.block_on(export(options))
}

fn parse(args: &[String]) -> Result<Options, String> {
    if args.is_empty() || args.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Err(HELP.into());
    }
    let mut db = None;
    let mut out = None;
    let mut format = Format::Json;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--db" => {
                index += 1;
                db = Some(PathBuf::from(
                    args.get(index).ok_or("--db exige um caminho")?,
                ));
            }
            "--out" => {
                index += 1;
                out = Some(PathBuf::from(
                    args.get(index).ok_or("--out exige um diretório")?,
                ));
            }
            "--format" => {
                index += 1;
                format = match args.get(index).map(String::as_str) {
                    Some("json") => Format::Json,
                    Some("yaml") => Format::Yaml,
                    Some(other) => return Err(format!("formato inválido '{other}' (json|yaml)")),
                    None => return Err("--format exige json ou yaml".into()),
                };
            }
            other => return Err(format!("argumento desconhecido '{other}'\n{HELP}")),
        }
        index += 1;
    }
    Ok(Options {
        db: db.ok_or("--db é obrigatório")?,
        out: out.ok_or("--out é obrigatório")?,
        format,
    })
}

async fn export(options: Options) -> Result<(), String> {
    if !options.db.is_file() {
        return Err(format!("banco não encontrado: {}", options.db.display()));
    }
    if options.out.exists() {
        let non_empty = fs::read_dir(&options.out)
            .map_err(|error| error.to_string())?
            .next()
            .is_some();
        if non_empty {
            return Err(format!(
                "diretório de saída não vazio: {}",
                options.out.display()
            ));
        }
    }
    let pool = open_read_only(&options.db).await?;
    let schema_version = v1_schema_version(&pool).await?;

    let workspaces = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, name, root_path FROM workspaces ORDER BY name, id",
    )
    .fetch_all(&pool)
    .await
    .map_err(|error| format!("workspaces: {error}"))?
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
    .map_err(|error| format!("recipes: {error}"))?
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

    let settings =
        sqlx::query_as::<_, (String, String)>("SELECT key, value FROM app_settings ORDER BY key")
            .fetch_all(&pool)
            .await
            .map_err(|error| format!("settings: {error}"))?
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
    fs::create_dir_all(&options.out).map_err(|error| error.to_string())?;

    let mut files = BTreeMap::new();
    let (bundle_name, bundle_bytes) = match options.format {
        Format::Json => (
            "export.json",
            serde_json::to_vec_pretty(&bundle).map_err(|error| error.to_string())?,
        ),
        Format::Yaml => (
            "export.yaml",
            serde_yaml::to_string(&bundle)
                .map_err(|error| error.to_string())?
                .into_bytes(),
        ),
    };
    write_hashed(&options.out, bundle_name, &bundle_bytes, &mut files)?;

    let recipes_dir = options.out.join("recipes");
    for recipe in &bundle.recipes {
        if let Some(yaml) = &recipe.yaml {
            fs::create_dir_all(&recipes_dir).map_err(|error| error.to_string())?;
            let name = format!("{}.yaml", safe_name(&recipe.id));
            write_hashed(&recipes_dir, &name, yaml.as_bytes(), &mut files)?;
        }
    }

    let manifest = Manifest {
        schema_version,
        exported_at,
        format: match options.format {
            Format::Json => "json".into(),
            Format::Yaml => "yaml".into(),
        },
        counts: Counts {
            workspaces: bundle.workspaces.len(),
            recipes: bundle.recipes.len(),
            settings: bundle.settings.len(),
        },
        files,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    fs::write(options.out.join("manifest.json"), manifest_bytes)
        .map_err(|error| error.to_string())?;

    println!(
        "exportado: {} workspace(s), {} receita(s), {} setting(s) em {}",
        bundle.workspaces.len(),
        bundle.recipes.len(),
        bundle.settings.len(),
        options.out.display()
    );
    Ok(())
}

async fn open_read_only(path: &Path) -> Result<SqlitePool, String> {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .read_only(true)
                .pragma("query_only", "true"),
        )
        .await
        .map_err(|error| format!("não foi possível abrir {}: {error}", path.display()))
}

async fn v1_schema_version(pool: &SqlitePool) -> Result<i64, String> {
    let has_migrations: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations'",
    )
    .fetch_one(pool)
    .await
    .map_err(|error| error.to_string())?;
    if has_migrations == 0 {
        return Err("o arquivo não tem _sqlx_migrations; não parece um banco OpenDesk V1".into());
    }
    let is_v2 =
        sqlx::query_scalar::<_, String>("SELECT value FROM v2_meta WHERE key = 'schema_version'")
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    if is_v2.is_some() {
        return Err("este banco já é V2; não há nada a exportar".into());
    }
    let version: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
        .map_err(|error| error.to_string())?;
    if version == 0 {
        return Err("banco sem migrations aplicadas; nada para exportar".into());
    }
    Ok(version)
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
) -> Result<(), String> {
    fs::write(dir.join(name), bytes).map_err(|error| error.to_string())?;
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
include!("export_tests.rs");
