//! Catalog listing: installed records, health, summaries and installable items.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    app_state::AppState,
    config_sync::{
        file_ops::{read_text, validate_slug},
        markdown,
    },
    db::{schedules, workspaces},
    error::{AppError, AppResult},
    recipes::parser as recipe_parser,
};

use super::{
    BUNDLE,
    bundle::{catalog, collect_dir, file_name, load_manifest},
    plan::planned_for,
    types::{
        Category, InstalledFile, InstalledRoutine, PackCounts, PackDetail, PackHealth, PackItem,
        PackManifest, PackSummary, parse_stored_routines,
    },
};

#[derive(Debug, Clone)]
struct InstalledRecord {
    version: String,
    installed_at: String,
    files: Vec<InstalledFile>,
    routines: Vec<InstalledRoutine>,
}

async fn installed_map(
    pool: &sqlx::SqlitePool,
    workspace_id: Option<&str>,
) -> AppResult<HashMap<String, InstalledRecord>> {
    let Some(workspace_id) = workspace_id else {
        return Ok(HashMap::new());
    };
    let rows = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT pack_id, pack_version, installed_at, files_json, routines_json
         FROM starter_pack_installs WHERE workspace_id = ?",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, version, installed_at, files_json, routines_json)| {
            (
                id,
                InstalledRecord {
                    version,
                    installed_at,
                    files: serde_json::from_str(&files_json).unwrap_or_default(),
                    routines: parse_stored_routines(&routines_json),
                },
            )
        })
        .collect())
}

fn health_for(root: &Path, record: &InstalledRecord, db_routines: &[String]) -> PackHealth {
    let mut health = PackHealth::default();
    for file in &record.files {
        match read_text(&root.join(&file.path)) {
            Ok(Some(snapshot)) if snapshot.sha256 == file.sha256 => {}
            Ok(Some(_)) => health.modified_files += 1,
            _ => health.missing_files += 1,
        }
    }
    for routine in &record.routines {
        if !db_routines.contains(&routine.id) {
            health.missing_routines += 1;
        }
    }
    health.status = if health.missing_files > 0 || health.missing_routines > 0 {
        "partial".into()
    } else if health.modified_files > 0 {
        "modified".into()
    } else {
        "installed".into()
    };
    health
}

async fn routine_ids(pool: &sqlx::SqlitePool, workspace_id: &str) -> AppResult<Vec<String>> {
    Ok(schedules::list(pool, workspace_id)
        .await?
        .into_iter()
        .map(|schedule| schedule.id)
        .collect())
}

fn summary(
    manifest: &PackManifest,
    installed: Option<&InstalledRecord>,
    health: PackHealth,
) -> PackSummary {
    PackSummary {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        name: manifest.name.clone(),
        category: manifest.category.clone(),
        description: manifest.description.clone(),
        icon: manifest.icon.clone(),
        color: manifest.color.clone(),
        recommended: manifest.recommended,
        requires: manifest.requires.clone(),
        counts: PackCounts {
            agents: manifest.contents.agents.len(),
            skills: manifest.contents.skills.len(),
            commands: manifest.contents.commands.len(),
            recipes: manifest.contents.recipes.len(),
            routines: manifest.contents.routines.len(),
        },
        installed: installed.is_some(),
        installed_version: installed.map(|value| value.version.clone()),
        installed_at: installed.map(|value| value.installed_at.clone()),
        update_available: installed
            .map(|value| value.version != manifest.version)
            .unwrap_or(false),
        health,
    }
}

pub async fn list(
    state: &Arc<AppState>,
    workspace_id: Option<&str>,
) -> AppResult<(Vec<Category>, Vec<PackSummary>)> {
    let catalog = catalog()?;
    let workspace_id = match workspace_id {
        Some(id) => Some(id.to_string()),
        None => crate::api::tasks::resolve_workspace_id(state, None)
            .await
            .ok(),
    };
    let installed = installed_map(&state.db, workspace_id.as_deref()).await?;
    let root = match workspace_id.as_deref() {
        Some(id) => workspaces::get(&state.db, id)
            .await
            .ok()
            .map(|workspace| PathBuf::from(workspace.root_path)),
        None => None,
    };
    let db_routines = match workspace_id.as_deref() {
        Some(id) => routine_ids(&state.db, id).await?,
        None => Vec::new(),
    };
    let mut packs = Vec::new();
    for id in &catalog.packs {
        let (manifest, _) = load_manifest(id)?;
        let record = installed.get(id);
        let health = match (root.as_deref(), record) {
            (Some(root), Some(record)) => health_for(root, record, &db_routines),
            _ => PackHealth::default(),
        };
        packs.push(summary(&manifest, record, health));
    }
    Ok((catalog.categories, packs))
}

pub async fn detail(
    state: &Arc<AppState>,
    pack_id: &str,
    workspace_id: Option<&str>,
) -> AppResult<PackDetail> {
    let (manifest, _) = load_manifest(pack_id)?;
    let workspace_id = match workspace_id {
        Some(id) => Some(id.to_string()),
        None => crate::api::tasks::resolve_workspace_id(state, None)
            .await
            .ok(),
    };
    let installed = installed_map(&state.db, workspace_id.as_deref()).await?;
    let entry = installed.get(pack_id);
    let health = match (entry, workspace_id.as_deref()) {
        (Some(record), Some(id)) => match workspaces::get(&state.db, id).await {
            Ok(workspace) => health_for(
                Path::new(&workspace.root_path),
                record,
                &routine_ids(&state.db, id).await?,
            ),
            Err(_) => PackHealth::default(),
        },
        _ => PackHealth::default(),
    };
    let update_available = entry
        .map(|value| value.version != manifest.version)
        .unwrap_or(false);
    Ok(PackDetail {
        manifest,
        installed: entry.is_some(),
        installed_version: entry.map(|value| value.version.clone()),
        installed_at: entry.map(|value| value.installed_at.clone()),
        update_available,
        health,
    })
}

/// Lists the individual installable items of a pack. This is read from the
/// embedded bundle, so the UI can inspect an item before installing anything.
pub fn items(pack_id: &str, workspace_root: Option<&Path>) -> AppResult<Vec<PackItem>> {
    let (manifest, dir) = load_manifest(pack_id)?;
    let planned = planned_for(&manifest, dir)?;
    let mut items = Vec::new();

    for name in &manifest.contents.agents {
        let name = file_name(name)?;
        let file = BUNDLE
            .get_file(format!("packs/{}/agents/{name}", manifest.id))
            .ok_or_else(|| AppError::BadRequest(format!("agente ausente: {name}")))?;
        let content = std::str::from_utf8(file.contents())
            .map_err(|_| AppError::BadRequest(format!("agente não é UTF-8: {name}")))?
            .to_string();
        let document = markdown::parse(&content)?;
        let id = Path::new(&name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(&name)
            .to_string();
        items.push(PackItem {
            kind: "agent".into(),
            id,
            name: markdown::get_string(&document.frontmatter, "name")
                .unwrap_or_else(|| name.clone()),
            description: markdown::get_string(&document.frontmatter, "description"),
            content,
            files: Vec::new(),
            installed: false,
            status: "missing".into(),
        });
    }

    for name in &manifest.contents.commands {
        let name = file_name(name)?;
        let file = BUNDLE
            .get_file(format!("packs/{}/commands/{name}", manifest.id))
            .ok_or_else(|| AppError::BadRequest(format!("comando ausente: {name}")))?;
        let content = std::str::from_utf8(file.contents())
            .map_err(|_| AppError::BadRequest(format!("comando não é UTF-8: {name}")))?
            .to_string();
        let document = markdown::parse(&content)?;
        let id = Path::new(&name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(&name)
            .to_string();
        items.push(PackItem {
            kind: "command".into(),
            id: id.clone(),
            name: format!("/{id}"),
            description: markdown::get_string(&document.frontmatter, "description"),
            content,
            files: Vec::new(),
            installed: false,
            status: "missing".into(),
        });
    }

    for id in &manifest.contents.skills {
        let id = validate_slug(id, false)?;
        let base = format!("packs/{}/skills/{id}", manifest.id);
        let file = BUNDLE
            .get_file(format!("{base}/SKILL.md"))
            .ok_or_else(|| AppError::BadRequest(format!("skill ausente: {id}")))?;
        let content = std::str::from_utf8(file.contents())
            .map_err(|_| AppError::BadRequest(format!("skill não é UTF-8: {id}")))?
            .to_string();
        let document = markdown::parse(&content)?;
        let skill_dir = BUNDLE
            .get_dir(&base)
            .ok_or_else(|| AppError::BadRequest(format!("skill ausente: {id}")))?;
        let mut files = Vec::new();
        collect_dir(skill_dir, "", &mut files);
        items.push(PackItem {
            kind: "skill".into(),
            id: id.clone(),
            name: markdown::get_string(&document.frontmatter, "name").unwrap_or(id.clone()),
            description: markdown::get_string(&document.frontmatter, "description"),
            content,
            files: files
                .into_iter()
                .map(|(path, _)| path.trim_start_matches('/').to_string())
                .filter(|path| path != "SKILL.md")
                .collect(),
            installed: false,
            status: "missing".into(),
        });
    }

    for name in &manifest.contents.recipes {
        let name = file_name(name)?;
        let file = BUNDLE
            .get_file(format!("packs/{}/recipes/{name}", manifest.id))
            .ok_or_else(|| AppError::BadRequest(format!("receita ausente: {name}")))?;
        let content = std::str::from_utf8(file.contents())
            .map_err(|_| AppError::BadRequest(format!("receita não é UTF-8: {name}")))?
            .to_string();
        let recipe = recipe_parser::parse(&content)?;
        items.push(PackItem {
            kind: "recipe".into(),
            id: recipe.id.clone(),
            name: recipe.name.clone(),
            description: recipe.description.clone(),
            content,
            files: Vec::new(),
            installed: false,
            status: "missing".into(),
        });
    }

    if let Some(root) = workspace_root {
        for item in &mut items {
            let skill_prefix = format!(".opencode/skills/{}/", item.id);
            let item_suffix = format!("/{}.md", item.id);
            let recipe_suffix = format!("/{}.yaml", item.id);
            let matching = planned.iter().filter(|file| match item.kind.as_str() {
                "agent" | "command" => file.target.ends_with(&item_suffix),
                "skill" => file.target.starts_with(&skill_prefix),
                "recipe" => file.target.ends_with(&recipe_suffix),
                _ => false,
            });
            let mut total = 0;
            let mut present = 0;
            let mut identical = 0;
            for file in matching {
                total += 1;
                if let Ok(Some(snapshot)) = read_text(&root.join(&file.target)) {
                    present += 1;
                    if snapshot.sha256 == file.sha256 {
                        identical += 1;
                    }
                }
            }
            item.installed = total > 0 && identical == total;
            item.status = if item.installed {
                "installed".into()
            } else if present > 0 {
                "modified".into()
            } else {
                "missing".into()
            };
        }
    }

    Ok(items)
}
