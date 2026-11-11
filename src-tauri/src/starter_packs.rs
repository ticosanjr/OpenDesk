use crate::{
    app_state::AppState,
    config_sync::{
        file_ops::{read_text, validate_slug, write_text_atomic},
        markdown,
        paths::StudioPaths,
    },
    db::{self, schedules, settings, workspaces},
    error::{AppError, AppResult},
    recipes::{parser as recipe_parser, store as recipe_store},
};
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

mod plan;

use plan::{load_pack, planned_for, planned_routines};

pub(super) static BUNDLE: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../starter-packs");

/// Pack installed in every newly created workspace. It carries the skill and
/// commands that teach agents to use the `opendesk-control` MCP. MCP bridges
/// themselves are registered for all workspaces by `proposals::ensure_all`.
pub const BASE_PACK_ID: &str = "controle-opendesk";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Requires {
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub integrations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackContents {
    #[serde(default)]
    pub agents: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub commands: Vec<String>,
    #[serde(default)]
    pub recipes: Vec<String>,
    #[serde(default)]
    pub routines: Vec<String>,
    #[serde(default)]
    pub folders: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackExample {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub recipe: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackManifest {
    pub schema: u32,
    pub id: String,
    pub version: String,
    pub name: String,
    pub category: String,
    pub description: String,
    #[serde(default = "default_icon")]
    pub icon: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default)]
    pub recommended: bool,
    #[serde(default)]
    pub requires: Requires,
    #[serde(default)]
    pub contents: PackContents,
    #[serde(default)]
    pub examples: Vec<PackExample>,
}

fn default_icon() -> String {
    "package".into()
}
fn default_color() -> String {
    "#3b82f6".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogFile {
    pub schema: u32,
    #[serde(default)]
    pub categories: Vec<Category>,
    pub packs: Vec<String>,
    pub templates: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackCounts {
    pub agents: usize,
    pub skills: usize,
    pub commands: usize,
    pub recipes: usize,
    pub routines: usize,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct PackHealth {
    /// installed | modified | partial
    pub status: String,
    pub missing_files: usize,
    pub modified_files: usize,
    pub missing_routines: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackSummary {
    pub id: String,
    pub version: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub icon: String,
    pub color: String,
    pub recommended: bool,
    pub requires: Requires,
    pub counts: PackCounts,
    pub installed: bool,
    pub installed_version: Option<String>,
    pub installed_at: Option<String>,
    pub update_available: bool,
    pub health: PackHealth,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackDetail {
    pub manifest: PackManifest,
    pub installed: bool,
    pub installed_at: Option<String>,
    pub installed_version: Option<String>,
    pub update_available: bool,
    pub health: PackHealth,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackItem {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub content: String,
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemInstallResult {
    pub kind: String,
    pub id: String,
    pub written: usize,
    pub skipped_conflicts: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanFile {
    pub path: String,
    pub kind: String,
    /// create | identical | conflict
    pub status: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlanRoutine {
    pub file: String,
    pub name: String,
    pub trigger_type: String,
    pub recipe_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallPlan {
    pub pack_id: String,
    pub version: String,
    pub files: Vec<PlanFile>,
    pub routines: Vec<PlanRoutine>,
    pub folders: Vec<String>,
    pub requires: Requires,
    pub conflicts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledFile {
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledRoutine {
    id: String,
    #[serde(default)]
    file: String,
    #[serde(default)]
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum StoredRoutine {
    Legacy(String),
    Full {
        id: String,
        #[serde(default)]
        file: String,
        #[serde(default)]
        name: String,
    },
}

fn parse_stored_routines(json: &str) -> Vec<InstalledRoutine> {
    serde_json::from_str::<Vec<StoredRoutine>>(json)
        .unwrap_or_default()
        .into_iter()
        .map(|routine| match routine {
            StoredRoutine::Legacy(id) => InstalledRoutine {
                id,
                file: String::new(),
                name: String::new(),
            },
            StoredRoutine::Full { id, file, name } => InstalledRoutine { id, file, name },
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallResult {
    pub pack_id: String,
    pub version: String,
    pub written: usize,
    pub skipped: usize,
    pub skipped_conflicts: usize,
    pub routines: Vec<String>,
    pub folders: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UninstallResult {
    pub pack_id: String,
    pub removed: usize,
    pub kept: Vec<String>,
    pub routines_removed: usize,
    pub routines_kept: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTemplate {
    pub schema: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_policy")]
    pub router_policy: String,
    pub packs: Vec<String>,
    #[serde(default)]
    pub folders: Vec<String>,
    #[serde(default)]
    pub readme: Option<String>,
    #[serde(default)]
    pub memory: Option<String>,
}

fn default_policy() -> String {
    "COST_BENEFIT".into()
}

pub(super) fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn catalog() -> AppResult<CatalogFile> {
    let file = BUNDLE
        .get_file("catalog.json")
        .ok_or_else(|| AppError::Internal("starter-packs/catalog.json ausente".into()))?;
    serde_json::from_slice(file.contents())
        .map_err(|error| AppError::Internal(format!("catalog.json inválido: {error}")))
}

fn pack_dir(id: &str) -> AppResult<&'static Dir<'static>> {
    BUNDLE
        .get_dir(format!("packs/{id}"))
        .ok_or_else(|| AppError::NotFound(format!("pacote {id}")))
}

fn load_manifest(id: &str) -> AppResult<(PackManifest, &'static Dir<'static>)> {
    let dir = pack_dir(id)?;
    let file = BUNDLE
        .get_file(format!("packs/{id}/pack.json"))
        .ok_or_else(|| AppError::NotFound(format!("manifesto do pacote {id}")))?;
    let manifest: PackManifest = serde_json::from_slice(file.contents())
        .map_err(|error| AppError::BadRequest(format!("manifesto inválido em {id}: {error}")))?;
    if manifest.id != id {
        return Err(AppError::BadRequest(format!(
            "manifesto declara id '{}' dentro de '{}'",
            manifest.id, id
        )));
    }
    Ok((manifest, dir))
}

pub(super) fn safe_relative(value: &str) -> AppResult<PathBuf> {
    let path = Path::new(value);
    if value.trim().is_empty()
        || path.is_absolute()
        || value.contains('\\')
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(AppError::BadRequest(format!(
            "caminho inválido no pacote: {value}"
        )));
    }
    Ok(path.to_path_buf())
}

pub(super) fn file_name(value: &str) -> AppResult<String> {
    safe_relative(value)?;
    Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| AppError::BadRequest(format!("nome de arquivo inválido: {value}")))
}

pub(super) fn collect_dir(
    dir: &'static Dir<'static>,
    prefix: &str,
    out: &mut Vec<(String, &'static [u8])>,
) {
    for file in dir.files() {
        if let Some(name) = file.path().file_name().and_then(|name| name.to_str()) {
            out.push((format!("{prefix}/{name}"), file.contents()));
        }
    }
    for child in dir.dirs() {
        if let Some(name) = child.path().file_name().and_then(|name| name.to_str()) {
            collect_dir(child, &format!("{prefix}/{name}"), out);
        }
    }
}

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
pub fn items(pack_id: &str) -> AppResult<Vec<PackItem>> {
    let (manifest, _) = load_manifest(pack_id)?;
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
        });
    }

    Ok(items)
}

/// Installs one item from a pack. The pack itself is not marked as installed;
/// item installs are intentionally independent.
pub async fn install_item(
    state: &Arc<AppState>,
    pack_id: &str,
    workspace_id: &str,
    kind: &str,
    id: &str,
    overwrite: bool,
) -> AppResult<ItemInstallResult> {
    let workspace = workspaces::get(&state.db, workspace_id).await?;
    let root = Path::new(&workspace.root_path);
    let (manifest, dir) = load_manifest(pack_id)?;
    let planned = planned_for(&manifest, dir)?;
    let item_prefix = format!(".opencode/skills/{id}/");
    let item_suffix = format!("/{id}.md");
    let item_recipe_suffix = format!("/{id}.yaml");
    let matching = planned
        .iter()
        .filter(|file| match kind {
            "agent" | "command" => file.target.ends_with(&item_suffix),
            "skill" => file.target.starts_with(&item_prefix),
            "recipe" => file.target.ends_with(&item_recipe_suffix),
            _ => false,
        })
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return Err(AppError::NotFound(format!(
            "item {kind} {id} no pacote {pack_id}"
        )));
    }

    let mut written = 0;
    let mut skipped_conflicts = 0;
    let mut recipes_changed = false;
    for file in matching {
        if let Some(snapshot) = read_text(&root.join(&file.target))?
            && snapshot.sha256 != file.sha256
            && !overwrite
        {
            skipped_conflicts += 1;
            continue;
        }
        let text = std::str::from_utf8(&file.bytes)
            .map_err(|_| AppError::Internal(format!("arquivo não é UTF-8: {}", file.target)))?;
        write_text_atomic(
            &root.join(&file.target),
            text,
            None,
            &state.paths.backups_dir,
        )?;
        if file.kind == "recipe" {
            recipes_changed = true;
        }
        written += 1;
    }
    if recipes_changed {
        recipe_store::sync_workspace(&state.db, workspace_id, root).await?;
        crate::api::recipes::sync_all_recipe_trigger_sources(state).await?;
    }
    if written == 0 && skipped_conflicts > 0 {
        return Err(AppError::Conflict(format!(
            "{kind} {id} já existe com conteúdo diferente; confirme a substituição"
        )));
    }
    Ok(ItemInstallResult {
        kind: kind.to_string(),
        id: id.to_string(),
        written,
        skipped_conflicts,
    })
}

pub async fn preview(
    state: &Arc<AppState>,
    pack_id: &str,
    workspace_id: &str,
) -> AppResult<InstallPlan> {
    let workspace = workspaces::get(&state.db, workspace_id).await?;
    let (manifest, dir) = load_manifest(pack_id)?;
    let loaded = load_pack(manifest, dir, workspace_id)?;
    let root = Path::new(&workspace.root_path);
    let mut files = Vec::new();
    let mut conflicts = 0;
    for planned in &loaded.planned {
        let status = match read_text(&root.join(&planned.target))? {
            Some(snapshot) if snapshot.sha256 == planned.sha256 => "identical",
            Some(_) => {
                conflicts += 1;
                "conflict"
            }
            None => "create",
        };
        files.push(PlanFile {
            path: planned.target.clone(),
            kind: planned.kind.clone(),
            status: status.to_string(),
            sha256: planned.sha256.clone(),
        });
    }
    let routines = loaded
        .routines
        .iter()
        .map(|(file, input)| PlanRoutine {
            file: file.clone(),
            name: input.name.clone(),
            trigger_type: input.trigger_type.clone(),
            recipe_id: input.recipe_id.clone(),
        })
        .collect();
    Ok(InstallPlan {
        pack_id: pack_id.to_string(),
        version: loaded.manifest.version,
        files,
        routines,
        folders: loaded.manifest.contents.folders.clone(),
        requires: loaded.manifest.requires.clone(),
        conflicts,
    })
}

pub async fn install(
    state: &Arc<AppState>,
    pack_id: &str,
    workspace_id: &str,
    overwrite: &[String],
    skip_conflicts: bool,
) -> AppResult<InstallResult> {
    let workspace = workspaces::get(&state.db, workspace_id).await?;
    let root = Path::new(&workspace.root_path);
    let (manifest, dir) = load_manifest(pack_id)?;
    let loaded = load_pack(manifest, dir, workspace_id)?;

    for folder in &loaded.manifest.contents.folders {
        safe_relative(folder)?;
        std::fs::create_dir_all(root.join(folder))?;
    }

    let overwrite: std::collections::HashSet<&str> = overwrite.iter().map(String::as_str).collect();
    let mut conflicts = Vec::new();
    for planned in &loaded.planned {
        if let Some(snapshot) = read_text(&root.join(&planned.target))?
            && snapshot.sha256 != planned.sha256
            && !overwrite.contains(planned.target.as_str())
        {
            conflicts.push(planned.target.clone());
        }
    }
    if !conflicts.is_empty() && !skip_conflicts {
        return Err(AppError::Conflict(format!(
            "estes arquivos já existem com conteúdo diferente: {}",
            conflicts.join(", ")
        )));
    }

    let mut written = 0usize;
    let mut skipped = 0usize;
    let mut skipped_conflicts = 0usize;
    let mut recipes_changed = false;
    for planned in &loaded.planned {
        if let Some(snapshot) = read_text(&root.join(&planned.target))?
            && snapshot.sha256 == planned.sha256
        {
            skipped += 1;
            continue;
        }
        if let Some(snapshot) = read_text(&root.join(&planned.target))?
            && snapshot.sha256 != planned.sha256
            && !overwrite.contains(planned.target.as_str())
        {
            skipped_conflicts += 1;
            continue;
        }
        let text = std::str::from_utf8(&planned.bytes)
            .map_err(|_| AppError::Internal(format!("arquivo não é UTF-8: {}", planned.target)))?;
        write_text_atomic(
            &root.join(&planned.target),
            text,
            None,
            &state.paths.backups_dir,
        )?;
        if planned.kind == "recipe" {
            recipes_changed = true;
        }
        written += 1;
    }
    if recipes_changed {
        recipe_store::sync_workspace(&state.db, workspace_id, root).await?;
        crate::api::recipes::sync_all_recipe_trigger_sources(state).await?;
    }

    let previous = sqlx::query_scalar::<_, String>(
        "SELECT routines_json FROM starter_pack_installs
         WHERE workspace_id = ? AND pack_id = ?",
    )
    .bind(workspace_id)
    .bind(&loaded.manifest.id)
    .fetch_optional(&state.db)
    .await?
    .map(|json| parse_stored_routines(&json))
    .unwrap_or_default();

    let mut installed_routines = Vec::new();
    for (index, (file, input)) in loaded.routines.iter().enumerate() {
        let mut existing_id = previous
            .iter()
            .find(|routine| !routine.file.is_empty() && routine.file == *file)
            .map(|routine| routine.id.clone());
        if existing_id.is_none()
            && let Some(routine) = previous.get(index)
            && routine.file.is_empty()
        {
            existing_id = Some(routine.id.clone());
        }
        if existing_id.is_none()
            && let Some(recipe_id) = input.recipe_id.as_deref()
        {
            if let Some(routine) =
                schedules::find_by_name_recipe(&state.db, workspace_id, &input.name, recipe_id)
                    .await?
            {
                existing_id = Some(routine.id);
            } else if let Some(routine) =
                schedules::find_by_recipe(&state.db, workspace_id, recipe_id).await?
            {
                existing_id = Some(routine.id);
            }
        }

        if let Some(id) = existing_id
            && schedules::get(&state.db, &id).await.is_ok()
        {
            let update = schedules::UpdateScheduleInput {
                name: Some(input.name.clone()),
                description: input.description.clone(),
                cron_expression: Some(input.cron_expression.clone()),
                timezone: Some(input.timezone.clone()),
                agent_name: input.agent_name.clone(),
                command_or_prompt: Some(input.command_or_prompt.clone()),
                trigger_type: Some(input.trigger_type.clone()),
                trigger_config_json: input.trigger_config_json.clone(),
                target_kind: Some(input.target_kind.clone()),
                recipe_id: input.recipe_id.clone(),
                router_policy: Some(input.router_policy.clone()),
                output_folder_template: Some(input.output_folder_template.clone()),
                notify_email_recipients: input.notify_email_recipients.clone(),
                notify_signal: Some(input.notify_signal),
                notify_signal_recipients: input.notify_signal_recipients.clone(),
                notify_only_on_change: Some(input.notify_only_on_change),
                recurrence_json: input.recurrence_json.clone(),
                one_time: Some(input.one_time),
                misfire_policy: Some(input.misfire_policy.clone()),
                safe_readonly_mode: Some(input.safe_readonly_mode),
                sync_to_google_docs: Some(input.sync_to_google_docs),
                sync_to_google_calendar: Some(input.sync_to_google_calendar),
                is_active: None,
            };
            let schedule = schedules::update(&state.db, &id, update).await?;
            crate::api::schedules::sync_trigger_source(state, &schedule).await?;
            installed_routines.push(InstalledRoutine {
                id: schedule.id,
                file: file.clone(),
                name: schedule.name,
            });
            continue;
        }

        let schedule = schedules::create(&state.db, input.clone()).await?;
        crate::api::schedules::sync_trigger_source(state, &schedule).await?;
        installed_routines.push(InstalledRoutine {
            id: schedule.id,
            file: file.clone(),
            name: schedule.name.clone(),
        });
    }
    let routine_ids: Vec<String> = installed_routines
        .iter()
        .map(|routine| routine.id.clone())
        .collect();

    let files_json = serde_json::to_string(
        &loaded
            .planned
            .iter()
            .map(|planned| InstalledFile {
                path: planned.target.clone(),
                sha256: planned.sha256.clone(),
            })
            .collect::<Vec<_>>(),
    )?;
    if !installed_routines.is_empty() {
        state.io_bus.reload(state.clone()).await?;
    }
    let routines_json = serde_json::to_string(&installed_routines)?;
    let id = uuid::Uuid::now_v7().to_string();
    sqlx::query(
        "INSERT INTO starter_pack_installs
            (id, workspace_id, pack_id, pack_version, files_json, routines_json)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(workspace_id, pack_id) DO UPDATE SET
            pack_version = excluded.pack_version,
            files_json = excluded.files_json,
            routines_json = excluded.routines_json,
            installed_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(&loaded.manifest.id)
    .bind(&loaded.manifest.version)
    .bind(files_json)
    .bind(routines_json)
    .execute(&state.db)
    .await?;

    crate::api::files::invalidate_search_index();
    if let Err(error) = state.refresh_watcher().await {
        tracing::warn!(error = %error, "watcher refresh after pack install failed");
    }

    Ok(InstallResult {
        pack_id: loaded.manifest.id,
        version: loaded.manifest.version,
        written,
        skipped,
        skipped_conflicts,
        routines: routine_ids,
        folders: loaded.manifest.contents.folders,
    })
}

/// Installs the operational baseline in a newly created workspace. Existing
/// files are never overwritten; conflicts are reported in the returned result
/// (or logged by callers that treat them as best effort).
pub async fn install_base_pack(
    state: &Arc<AppState>,
    workspace_id: &str,
) -> AppResult<InstallResult> {
    install(state, BASE_PACK_ID, workspace_id, &[], true).await
}

pub async fn uninstall(
    state: &Arc<AppState>,
    pack_id: &str,
    workspace_id: &str,
) -> AppResult<UninstallResult> {
    let workspace = workspaces::get(&state.db, workspace_id).await?;
    let root = Path::new(&workspace.root_path);
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT files_json, routines_json FROM starter_pack_installs
         WHERE workspace_id = ? AND pack_id = ?",
    )
    .bind(workspace_id)
    .bind(pack_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("pacote {pack_id} não instalado")))?;
    let files: Vec<InstalledFile> = serde_json::from_str(&row.0).unwrap_or_default();
    let routines = parse_stored_routines(&row.1);

    // Routines that already ran (or were activated) are kept, together with
    // their recipe file, so the user never ends up with a broken routine.
    let mut routines_removed = 0usize;
    let mut routines_kept = Vec::new();
    let mut kept_recipe_targets = std::collections::HashSet::new();
    for routine in routines {
        let routine_id = routine.id;
        if let Ok(schedule) = schedules::get(&state.db, &routine_id).await {
            if schedule.last_run_at.is_some() || schedule.is_active {
                if let Some(recipe_id) = schedule.recipe_id.clone() {
                    kept_recipe_targets.insert(format!(".opencode/recipes/{recipe_id}.yaml"));
                }
                routines_kept.push(routine_id);
            } else {
                let _ = crate::api::schedules::remove_trigger_sources(state, &routine_id).await;
                schedules::delete(&state.db, &routine_id).await?;
                routines_removed += 1;
            }
        }
    }

    let mut removed = 0usize;
    let mut kept = Vec::new();
    for file in files {
        if kept_recipe_targets.contains(&file.path) {
            kept.push(file.path);
            continue;
        }
        let path = root.join(&file.path);
        match read_text(&path)? {
            Some(snapshot) if snapshot.sha256 == file.sha256 => {
                std::fs::remove_file(&path)?;
                removed += 1;
            }
            Some(_) => kept.push(file.path),
            None => {}
        }
    }

    if removed > 0 {
        // Keep the recipe index in step with files removed from disk.
        let _ = crate::recipes::store::sync_workspace(&state.db, workspace_id, root).await;
    }
    sqlx::query("DELETE FROM starter_pack_installs WHERE workspace_id = ? AND pack_id = ?")
        .bind(workspace_id)
        .bind(pack_id)
        .execute(&state.db)
        .await?;
    let _ = crate::api::recipes::sync_all_recipe_trigger_sources(state).await;

    crate::api::files::invalidate_search_index();
    if let Err(error) = state.refresh_watcher().await {
        tracing::warn!(error = %error, "watcher refresh after pack uninstall failed");
    }

    Ok(UninstallResult {
        pack_id: pack_id.to_string(),
        removed,
        kept,
        routines_removed,
        routines_kept,
    })
}

/// Create the first-run capability factory workspace once. Existing users keep
/// their active workspace; the factory is additive and fully removable.
pub async fn ensure_default_factory_workspace(
    state: &Arc<AppState>,
) -> AppResult<Option<workspaces::Workspace>> {
    if settings::get(&state.db, "factory_workspace_created")
        .await?
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return Ok(None);
    }
    let existing = workspaces::list(&state.db).await?;
    let had_workspaces = !existing.is_empty();
    let workspace = if let Some(found) = existing
        .iter()
        .find(|workspace| workspace.name == "Fábrica de Capacidades")
        .cloned()
    {
        found
    } else {
        let root_setting = settings::get(&state.db, "workspaces_root")
            .await?
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| {
                crate::paths::default_workspaces_root()
                    .to_string_lossy()
                    .to_string()
            });
        let root = PathBuf::from(root_setting).join("fabrica-de-capacidades");
        workspaces::create_trusted(
            &state.db,
            &state.paths,
            db::models::CreateWorkspaceInput {
                name: "Fábrica de Capacidades".into(),
                root_path: root.to_string_lossy().to_string(),
                description: Some(
                    "Crie, avalie e melhore skills, agentes e comandos com metodologia eval-driven."
                        .into(),
                ),
                color_hex: Some("#8b5cf6".into()),
                default_router_policy: Some("COST_BENEFIT".into()),
                default_email_recipient: None,
                google_drive_folder_id: None,
                timezone: None,
            },
        )
        .await?
    };

    let template = templates()?
        .into_iter()
        .find(|template| template.id == "fabrica-capacidades");
    if let Some(template) = &template {
        for folder in &template.folders {
            safe_relative(folder)?;
            std::fs::create_dir_all(Path::new(&workspace.root_path).join(folder))?;
        }
        for (filename, content) in [
            ("README.md", template.readme.as_deref()),
            ("MEMORY.md", template.memory.as_deref()),
        ] {
            if let Some(content) = content {
                let path = Path::new(&workspace.root_path).join(filename);
                if !path.exists() {
                    std::fs::write(path, content)?;
                }
            }
        }
    }

    let packs = template.map(|template| template.packs).unwrap_or_else(|| {
        vec![
            BASE_PACK_ID.into(),
            "essenciais".into(),
            "meta-forge".into(),
        ]
    });
    for pack in packs {
        if let Err(error) = install(state, &pack, &workspace.id, &[], true).await {
            tracing::warn!(pack = %pack, error = %error, "default factory pack install failed");
            return Err(error);
        }
    }
    if !had_workspaces {
        settings::set_active_workspace(&state.db, &workspace.id).await?;
    }
    if let Err(error) = crate::proposals::ensure_workspace_bridge(state, &workspace.id).await {
        tracing::warn!(error = %error, "could not register config bridge for factory workspace");
    }
    settings::set(
        &state.db,
        "factory_workspace_created",
        &serde_json::Value::Bool(true),
    )
    .await?;
    if let Err(error) = state.refresh_watcher().await {
        tracing::warn!(error = %error, "watcher refresh after factory workspace failed");
    }
    Ok(Some(workspace))
}

pub fn templates() -> AppResult<Vec<WorkspaceTemplate>> {
    let catalog = catalog()?;
    let mut list = Vec::new();
    for id in &catalog.templates {
        let file = BUNDLE
            .get_file(format!("templates/{id}.json"))
            .ok_or_else(|| AppError::NotFound(format!("template {id}")))?;
        let template: WorkspaceTemplate = serde_json::from_slice(file.contents())
            .map_err(|error| AppError::BadRequest(format!("template inválido {id}: {error}")))?;
        list.push(template);
    }
    Ok(list)
}

#[derive(Debug, Clone, Serialize)]
pub struct TemplateResult {
    pub workspace: workspaces::Workspace,
    pub installed: Vec<String>,
    pub errors: Vec<String>,
}

pub async fn create_from_template(
    state: &Arc<AppState>,
    template_id: &str,
    name: Option<&str>,
    root_path: Option<&str>,
    color: Option<&str>,
) -> AppResult<TemplateResult> {
    let template = templates()?
        .into_iter()
        .find(|template| template.id == template_id)
        .ok_or_else(|| AppError::NotFound(format!("template {template_id}")))?;
    let name = name
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&template.name)
        .to_string();
    let root = match root_path.map(str::trim).filter(|value| !value.is_empty()) {
        Some(path) => PathBuf::from(path),
        None => crate::paths::default_workspaces_root().join(crate::dsh_router::slugify(&name)),
    };
    std::fs::create_dir_all(&root)?;
    let workspace = workspaces::create(
        &state.db,
        &state.paths,
        db::models::CreateWorkspaceInput {
            name: name.clone(),
            root_path: root.to_string_lossy().to_string(),
            description: Some(template.description.clone()),
            color_hex: Some(color.unwrap_or(&template.color).to_string()),
            default_router_policy: Some(template.router_policy.clone()),
            default_email_recipient: None,
            google_drive_folder_id: None,
            timezone: None,
        },
    )
    .await?;
    crate::db::settings::set_active_workspace(&state.db, &workspace.id).await?;
    if let Err(error) = crate::proposals::ensure_workspace_bridge(state, &workspace.id).await {
        tracing::warn!(error = %error, "could not register config bridge for template workspace");
    }

    for folder in &template.folders {
        safe_relative(folder)?;
        std::fs::create_dir_all(Path::new(&workspace.root_path).join(folder))?;
    }
    for (filename, content) in [
        ("README.md", template.readme.as_deref()),
        ("MEMORY.md", template.memory.as_deref()),
    ] {
        if let Some(content) = content {
            let path = Path::new(&workspace.root_path).join(filename);
            if !path.exists() {
                std::fs::write(path, content)?;
            }
        }
    }

    let mut installed = Vec::new();
    let mut errors = Vec::new();
    match install_base_pack(state, &workspace.id).await {
        Ok(result) => installed.push(result.pack_id),
        Err(error) => errors.push(format!("{BASE_PACK_ID}: {error}")),
    }
    for pack_id in &template.packs {
        match install(state, pack_id, &workspace.id, &[], false).await {
            Ok(result) => installed.push(result.pack_id),
            Err(error) => errors.push(format!("{pack_id}: {error}")),
        }
    }
    if let Err(error) = state.refresh_watcher().await {
        tracing::warn!(error = %error, "watcher refresh after template create failed");
    }
    Ok(TemplateResult {
        workspace,
        installed,
        errors,
    })
}

/// Validate every bundled pack. Used by tests and by the release gate.
#[allow(dead_code)]
pub fn validate_bundle() -> AppResult<Vec<String>> {
    let catalog = catalog()?;
    let mut issues = Vec::new();
    for id in &catalog.packs {
        match load_manifest(id) {
            Ok((manifest, dir)) => {
                if let Err(error) = planned_for(&manifest, dir) {
                    issues.push(format!("{id}: {error}"));
                }
                if let Err(error) = planned_routines(&manifest, dir, "validation") {
                    issues.push(format!("{id}: {error}"));
                }
            }
            Err(error) => issues.push(format!("{id}: {error}")),
        }
    }
    for id in &catalog.templates {
        if BUNDLE.get_file(format!("templates/{id}.json")).is_none() {
            issues.push(format!("template ausente: {id}"));
        }
    }
    if issues.is_empty() {
        Ok(Vec::new())
    } else {
        Err(AppError::Internal(issues.join("; ")))
    }
}

#[allow(dead_code)]
pub fn studio_paths(root: &Path) -> StudioPaths {
    StudioPaths::new(Some(root.to_path_buf()))
}

#[cfg(test)]
#[path = "starter_packs_tests.rs"]
mod tests;
