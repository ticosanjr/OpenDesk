use crate::{
    app_state::AppState,
    config_sync::{
        file_ops::{read_text, validate_slug, write_text_atomic},
        paths::StudioPaths,
    },
    db::{self, schedules, workspaces},
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

static BUNDLE: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../starter-packs");

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
}

#[derive(Debug, Clone, Serialize)]
pub struct PackDetail {
    pub manifest: PackManifest,
    pub installed: bool,
    pub installed_at: Option<String>,
    pub installed_version: Option<String>,
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

#[derive(Debug, Clone)]
struct Planned {
    target: String,
    kind: String,
    bytes: Vec<u8>,
    sha256: String,
    recipe_id: Option<String>,
}

struct LoadedPack {
    manifest: PackManifest,
    planned: Vec<Planned>,
    routines: Vec<(String, schedules::CreateScheduleInput)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstalledFile {
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallResult {
    pub pack_id: String,
    pub version: String,
    pub written: usize,
    pub skipped: usize,
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

fn sha256(bytes: &[u8]) -> String {
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

fn safe_relative(value: &str) -> AppResult<PathBuf> {
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

fn file_name(value: &str) -> AppResult<String> {
    safe_relative(value)?;
    Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .ok_or_else(|| AppError::BadRequest(format!("nome de arquivo inválido: {value}")))
}

fn collect_dir(dir: &'static Dir<'static>, prefix: &str, out: &mut Vec<(String, &'static [u8])>) {
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

fn planned_for(manifest: &PackManifest, _dir: &'static Dir<'static>) -> AppResult<Vec<Planned>> {
    let mut planned = Vec::new();
    let mut push = |target: String, kind: &str, bytes: &[u8], recipe_id: Option<String>| {
        planned.push(Planned {
            target,
            kind: kind.to_string(),
            sha256: sha256(bytes),
            bytes: bytes.to_vec(),
            recipe_id,
        });
    };

    for name in &manifest.contents.agents {
        let name = file_name(name)?;
        let file = BUNDLE
            .get_file(format!("packs/{}/agents/{name}", manifest.id))
            .ok_or_else(|| AppError::BadRequest(format!("agente ausente: {name}")))?;
        push(
            format!(".opencode/agents/{name}"),
            "agent",
            file.contents(),
            None,
        );
    }
    for name in &manifest.contents.commands {
        let name = file_name(name)?;
        let file = BUNDLE
            .get_file(format!("packs/{}/commands/{name}", manifest.id))
            .ok_or_else(|| AppError::BadRequest(format!("comando ausente: {name}")))?;
        push(
            format!(".opencode/commands/{name}"),
            "command",
            file.contents(),
            None,
        );
    }
    for id in &manifest.contents.skills {
        let id = validate_slug(id, false)?;
        let skill_dir = BUNDLE
            .get_dir(format!("packs/{}/skills/{id}", manifest.id))
            .ok_or_else(|| AppError::BadRequest(format!("skill ausente: {id}")))?;
        let mut files = Vec::new();
        collect_dir(skill_dir, "", &mut files);
        if files.is_empty() {
            return Err(AppError::BadRequest(format!("skill vazia: {id}")));
        }
        for (relative, bytes) in files {
            let relative = relative.trim_start_matches('/');
            safe_relative(relative)?;
            push(
                format!(".opencode/skills/{id}/{relative}"),
                "skill",
                bytes,
                None,
            );
        }
    }
    for name in &manifest.contents.recipes {
        let name = file_name(name)?;
        let file = BUNDLE
            .get_file(format!("packs/{}/recipes/{name}", manifest.id))
            .ok_or_else(|| AppError::BadRequest(format!("receita ausente: {name}")))?;
        let text = std::str::from_utf8(file.contents())
            .map_err(|_| AppError::BadRequest(format!("receita não é UTF-8: {name}")))?;
        let recipe = recipe_parser::parse(text)?;
        let id = validate_slug(&recipe.id, false)?;
        push(
            format!(".opencode/recipes/{id}.yaml"),
            "recipe",
            file.contents(),
            Some(id),
        );
    }
    for folder in &manifest.contents.folders {
        safe_relative(folder)?;
    }
    Ok(planned)
}

fn planned_routines(
    manifest: &PackManifest,
    _dir: &'static Dir<'static>,
    workspace_id: &str,
) -> AppResult<Vec<(String, schedules::CreateScheduleInput)>> {
    let mut routines = Vec::new();
    for name in &manifest.contents.routines {
        let name = file_name(name)?;
        let file = BUNDLE
            .get_file(format!("packs/{}/routines/{name}", manifest.id))
            .ok_or_else(|| AppError::BadRequest(format!("rotina ausente: {name}")))?;
        let mut value: serde_json::Value = serde_json::from_slice(file.contents())
            .map_err(|error| AppError::BadRequest(format!("rotina inválida {name}: {error}")))?;
        value["workspace_id"] = serde_json::Value::String(workspace_id.to_string());
        value["is_active"] = serde_json::Value::Bool(false);
        let input: schedules::CreateScheduleInput = serde_json::from_value(value)
            .map_err(|error| AppError::BadRequest(format!("rotina inválida {name}: {error}")))?;
        routines.push((name, input));
    }
    Ok(routines)
}

fn load_pack(
    manifest: PackManifest,
    dir: &'static Dir<'static>,
    workspace_id: &str,
) -> AppResult<LoadedPack> {
    let planned = planned_for(&manifest, dir)?;
    let routines = planned_routines(&manifest, dir, workspace_id)?;
    Ok(LoadedPack {
        manifest,
        planned,
        routines,
    })
}

async fn installed_map(
    pool: &sqlx::SqlitePool,
    workspace_id: Option<&str>,
) -> AppResult<HashMap<String, (String, String)>> {
    let Some(workspace_id) = workspace_id else {
        return Ok(HashMap::new());
    };
    let rows = sqlx::query_as::<_, (String, String, String)>(
        "SELECT pack_id, pack_version, installed_at FROM starter_pack_installs
         WHERE workspace_id = ?",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, version, installed_at)| (id, (version, installed_at)))
        .collect())
}

fn summary(manifest: &PackManifest, installed: Option<&(String, String)>) -> PackSummary {
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
        installed_version: installed.map(|value| value.0.clone()),
        installed_at: installed.map(|value| value.1.clone()),
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
    let mut packs = Vec::new();
    for id in &catalog.packs {
        let (manifest, _) = load_manifest(id)?;
        packs.push(summary(&manifest, installed.get(id)));
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
    Ok(PackDetail {
        manifest,
        installed: entry.is_some(),
        installed_version: entry.map(|value| value.0.clone()),
        installed_at: entry.map(|value| value.1.clone()),
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
    if !conflicts.is_empty() {
        return Err(AppError::Conflict(format!(
            "estes arquivos já existem com conteúdo diferente: {}",
            conflicts.join(", ")
        )));
    }

    let mut written = 0usize;
    let mut skipped = 0usize;
    for planned in &loaded.planned {
        if let Some(snapshot) = read_text(&root.join(&planned.target))?
            && snapshot.sha256 == planned.sha256
        {
            skipped += 1;
            continue;
        }
        if planned.kind == "recipe"
            && let Some(recipe_id) = &planned.recipe_id
        {
            let yaml = std::str::from_utf8(&planned.bytes)
                .map_err(|_| AppError::Internal("receita não é UTF-8".into()))?;
            recipe_store::save_yaml(&state.db, workspace_id, recipe_id, yaml).await?;
            written += 1;
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
        written += 1;
    }

    let mut routine_ids = Vec::new();
    for (_, input) in &loaded.routines {
        let schedule = schedules::create(&state.db, input.clone()).await?;
        crate::api::schedules::sync_trigger_source(state, &schedule).await?;
        routine_ids.push(schedule.id);
    }

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
    let routines_json = serde_json::to_string(&routine_ids)?;
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

    if let Err(error) = state.refresh_watcher().await {
        tracing::warn!(error = %error, "watcher refresh after pack install failed");
    }

    Ok(InstallResult {
        pack_id: loaded.manifest.id,
        version: loaded.manifest.version,
        written,
        skipped,
        routines: routine_ids,
        folders: loaded.manifest.contents.folders,
    })
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
    let routines: Vec<String> = serde_json::from_str(&row.1).unwrap_or_default();

    // Routines that already ran (or were activated) are kept, together with
    // their recipe file, so the user never ends up with a broken routine.
    let mut routines_removed = 0usize;
    let mut routines_kept = Vec::new();
    let mut kept_recipe_targets = std::collections::HashSet::new();
    for routine_id in routines {
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

    sqlx::query("DELETE FROM starter_pack_installs WHERE workspace_id = ? AND pack_id = ?")
        .bind(workspace_id)
        .bind(pack_id)
        .execute(&state.db)
        .await?;

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
    for pack_id in &template.packs {
        match install(state, pack_id, &workspace.id, &[]).await {
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
mod tests {
    use super::*;

    #[test]
    fn bundle_manifests_and_recipes_are_valid() {
        let issues = validate_bundle().expect("bundle must parse");
        assert!(issues.is_empty(), "issues: {issues:?}");
        let packs = catalog().expect("catalog").packs;
        assert!(
            packs.len() >= 20,
            "expected a broad catalog, got {}",
            packs.len()
        );
    }

    #[test]
    fn every_skill_has_a_skill_md() {
        for id in catalog().expect("catalog").packs {
            let (manifest, dir) = load_manifest(&id).expect("manifest");
            for skill in manifest.contents.skills {
                let _ = dir;
                assert!(
                    BUNDLE
                        .get_file(format!("packs/{id}/skills/{skill}/SKILL.md"))
                        .is_some(),
                    "{id}: {skill} sem SKILL.md"
                );
            }
        }
    }

    async fn test_state() -> (tempfile::TempDir, Arc<AppState>, String) {
        use crate::paths::AppPaths;
        let temp = tempfile::TempDir::new().expect("temp");
        let root = temp.path();
        let paths = AppPaths {
            data_dir: root.join("data"),
            config_dir: root.join("config"),
            cache_dir: root.join("cache"),
            log_dir: root.join("logs"),
            db_path: root.join("data").join("opendesk.db"),
            backups_dir: root.join("backups"),
            artifacts_dir: root.join("artifacts"),
            run_logs_dir: root.join("runs"),
            daemon_lock_path: root.join("daemon.json"),
            api_lock_path: root.join("api.json"),
        };
        for dir in [
            &paths.data_dir,
            &paths.config_dir,
            &paths.cache_dir,
            &paths.log_dir,
            &paths.backups_dir,
            &paths.artifacts_dir,
            &paths.run_logs_dir,
        ] {
            std::fs::create_dir_all(dir).expect("dir");
        }
        let workspace_root = root.join("workspace");
        std::fs::create_dir_all(&workspace_root).expect("workspace root");
        let state = Arc::new(
            crate::app_state::AppState::initialize(paths)
                .await
                .expect("state"),
        );
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-pack', 'Pack', ?)")
            .bind(workspace_root.to_string_lossy().to_string())
            .execute(&state.db)
            .await
            .expect("workspace");
        (temp, state, "ws-pack".to_string())
    }

    #[tokio::test]
    async fn installs_validates_and_uninstalls_a_pack() {
        let (_temp, state, workspace_id) = test_state().await;
        let workspace = workspaces::get(&state.db, &workspace_id).await.expect("ws");
        let root = PathBuf::from(&workspace.root_path);

        let result = install(&state, "essenciais", &workspace_id, &[])
            .await
            .expect("install");
        assert!(result.written >= 8, "written: {result:?}");
        assert!(root.join(".opencode/agents/assistente.md").is_file());
        assert!(
            root.join(".opencode/skills/pesquisa-web/SKILL.md")
                .is_file()
        );
        assert!(
            root.join(".opencode/recipes/briefing-diario.yaml")
                .is_file()
        );
        crate::recipes::store::get(&state.db, "briefing-diario")
            .await
            .expect("recipe indexed");

        let again = install(&state, "essenciais", &workspace_id, &[])
            .await
            .expect("reinstall");
        assert_eq!(again.written, 0);
        assert!(again.skipped >= 8);

        let agent_path = root.join(".opencode/agents/assistente.md");
        std::fs::write(&agent_path, "editado pelo usuário").expect("edit");

        let removed = uninstall(&state, "essenciais", &workspace_id)
            .await
            .expect("uninstall");
        assert!(
            removed
                .kept
                .iter()
                .any(|path| path.ends_with("assistente.md"))
        );
        assert!(agent_path.exists(), "modified file must be preserved");
        assert!(!root.join(".opencode/agents/pesquisador.md").exists());
    }

    #[tokio::test]
    async fn installs_routines_inactive_and_removes_them() {
        let (_temp, state, workspace_id) = test_state().await;
        let result = install(&state, "wiki-karpathy", &workspace_id, &[])
            .await
            .expect("install");
        assert_eq!(result.routines.len(), 1);
        let routine = schedules::get(&state.db, &result.routines[0])
            .await
            .expect("routine");
        assert!(!routine.is_active);
        assert_eq!(routine.target_kind, "RECIPE");
        let removed = uninstall(&state, "wiki-karpathy", &workspace_id)
            .await
            .expect("uninstall");
        assert_eq!(removed.routines_removed, 1);
    }

    #[tokio::test]
    async fn creates_a_workspace_from_a_template() {
        let (temp, state, _) = test_state().await;
        let root = temp.path().join("template-workspace");
        let result = create_from_template(
            &state,
            "conteudo",
            Some("Conteúdo Teste"),
            Some(root.to_string_lossy().as_ref()),
            Some("#8b5cf6"),
        )
        .await
        .expect("template");
        assert!(result.errors.is_empty(), "errors: {:?}", result.errors);
        assert_eq!(
            result.installed.len(),
            5,
            "installed: {:?}",
            result.installed
        );
        assert!(root.join("README.md").is_file());
        assert!(
            root.join(".opencode/agents/roteirista-youtube.md")
                .is_file()
        );
        assert!(root.join("content/youtube/input").is_dir());
    }

    #[test]
    fn workspace_templates_do_not_collide() {
        for template in templates().expect("templates") {
            let mut seen: HashMap<String, String> = HashMap::new();
            for pack_id in &template.packs {
                let (manifest, dir) = load_manifest(pack_id).expect("manifest");
                for planned in planned_for(&manifest, dir).expect("planned") {
                    if let Some(previous) = seen.insert(planned.target.clone(), pack_id.clone())
                        && previous != *pack_id
                    {
                        panic!(
                            "template {}: {} definido por {} e {}",
                            template.id, planned.target, previous, pack_id
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn rejects_path_traversal_in_folders() {
        assert!(safe_relative("../etc").is_err());
        assert!(safe_relative("/etc").is_err());
        assert!(safe_relative("input/../etc").is_err());
        assert!(safe_relative("input/dropzone").is_ok());
    }
}
