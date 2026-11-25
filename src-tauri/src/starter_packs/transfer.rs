//! Portable starter-pack transfers. Export embeds manifest plus every text
//! file; import validates all paths and contents before writing a single file.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    app_state::AppState,
    config_sync::{file_ops::validate_slug, markdown},
    db::{schedules, workspaces},
    error::{AppError, AppResult},
    recipes::{parser as recipe_parser, store as recipe_store},
};

use super::{
    bundle::{collect_dir, file_name, load_manifest, safe_relative},
    types::PackManifest,
};

const MAX_FILE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackFile {
    pub path: String,
    /// UTF-8 text or base64 for binary assets.
    pub content: String,
    #[serde(default = "default_encoding")]
    pub encoding: String,
}

fn default_encoding() -> String {
    "utf8".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackTransfer {
    pub manifest: PackManifest,
    pub files: Vec<PackFile>,
}

#[derive(Debug, Serialize)]
pub struct PackImportResult {
    pub pack_id: String,
    pub written: usize,
    pub routines: usize,
}

fn decode(file: &PackFile) -> AppResult<Vec<u8>> {
    let bytes = match file.encoding.as_str() {
        "utf8" => file.content.as_bytes().to_vec(),
        "base64" => STANDARD.decode(file.content.trim()).map_err(|error| {
            AppError::BadRequest(format!("base64 inválido em {}: {error}", file.path))
        })?,
        other => {
            return Err(AppError::BadRequest(format!(
                "encoding desconhecido em {}: {other}",
                file.path
            )));
        }
    };
    if bytes.len() > MAX_FILE_BYTES {
        return Err(AppError::BadRequest(format!(
            "arquivo {} excede 2 MB",
            file.path
        )));
    }
    Ok(bytes)
}

fn text(file: &PackFile) -> AppResult<String> {
    let bytes = decode(file)?;
    std::str::from_utf8(&bytes)
        .map(str::to_string)
        .map_err(|_| AppError::BadRequest(format!("arquivo não é UTF-8: {}", file.path)))
}

fn parts(path: &str) -> AppResult<(String, Vec<String>)> {
    let relative = safe_relative(path)?;
    let mut components = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if components.is_empty() {
        return Err(AppError::BadRequest(format!("caminho vazio: {path}")));
    }
    let kind = components.remove(0);
    Ok((kind, components))
}

/// Exports one bundled pack as portable JSON.
pub fn export(pack_id: &str) -> AppResult<PackTransfer> {
    let (manifest, dir) = load_manifest(pack_id)?;
    let mut collected = Vec::new();
    collect_dir(dir, "", &mut collected);
    let mut files = Vec::new();
    for (path, bytes) in collected {
        let path = path.trim_start_matches('/').to_string();
        let supported = ["agents/", "commands/", "skills/", "recipes/", "routines/"];
        if path == "pack.json" || !supported.iter().any(|prefix| path.starts_with(prefix)) {
            continue;
        }
        let (content, encoding) = match std::str::from_utf8(bytes) {
            Ok(text) => (text.to_string(), "utf8".to_string()),
            Err(_) => (STANDARD.encode(bytes), "base64".to_string()),
        };
        files.push(PackFile {
            path,
            content,
            encoding,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(PackTransfer { manifest, files })
}

/// Validates the whole transfer before any write. Fail-closed by design.
pub fn validate(transfer: &PackTransfer) -> AppResult<()> {
    let manifest = &transfer.manifest;
    let id = validate_slug(&manifest.id, false)?;
    if id != manifest.id {
        return Err(AppError::BadRequest(format!(
            "id de pacote inválido: {}",
            manifest.id
        )));
    }
    let agents = manifest
        .contents
        .agents
        .iter()
        .map(|name| file_name(name))
        .collect::<AppResult<HashSet<_>>>()?;
    let commands = manifest
        .contents
        .commands
        .iter()
        .map(|name| file_name(name))
        .collect::<AppResult<HashSet<_>>>()?;
    let skills = manifest
        .contents
        .skills
        .iter()
        .map(|skill| {
            validate_slug(skill, false).map_err(|error| AppError::BadRequest(error.to_string()))
        })
        .collect::<AppResult<HashSet<_>>>()?;
    let recipes = manifest
        .contents
        .recipes
        .iter()
        .map(|name| file_name(name))
        .collect::<AppResult<HashSet<_>>>()?;
    let routines = manifest
        .contents
        .routines
        .iter()
        .map(|name| file_name(name))
        .collect::<AppResult<HashSet<_>>>()?;
    for file in &transfer.files {
        let (kind, rest) = parts(&file.path)?;
        match kind.as_str() {
            "agents" => {
                let name = rest
                    .first()
                    .ok_or_else(|| AppError::BadRequest("agente sem nome".into()))?;
                if rest.len() != 1 || !agents.contains(name) {
                    return Err(AppError::BadRequest(format!(
                        "agente não declarado no manifesto: {}",
                        file.path
                    )));
                }
                if markdown::parse(&text(file)?)?.body.trim().is_empty() {
                    return Err(AppError::BadRequest(format!(
                        "agente sem instruções: {}",
                        file.path
                    )));
                }
            }
            "commands" => {
                let name = rest
                    .first()
                    .ok_or_else(|| AppError::BadRequest("comando sem nome".into()))?;
                if rest.len() != 1 || !commands.contains(name) {
                    return Err(AppError::BadRequest(format!(
                        "comando não declarado no manifesto: {}",
                        file.path
                    )));
                }
                if markdown::parse(&text(file)?)?.body.trim().is_empty() {
                    return Err(AppError::BadRequest(format!(
                        "comando sem instruções: {}",
                        file.path
                    )));
                }
            }
            "skills" => {
                let skill = rest
                    .first()
                    .ok_or_else(|| AppError::BadRequest("skill sem id".into()))?;
                if !skills.contains(skill) {
                    return Err(AppError::BadRequest(format!(
                        "skill não declarada no manifesto: {skill}"
                    )));
                }
                if rest.last().map(String::as_str) == Some("SKILL.md") {
                    let document = markdown::parse(&text(file)?)?;
                    if markdown::get_string(&document.frontmatter, "description")
                        .unwrap_or_default()
                        .trim()
                        .is_empty()
                    {
                        return Err(AppError::BadRequest(format!(
                            "SKILL.md sem description: {}",
                            file.path
                        )));
                    }
                }
            }
            "recipes" => {
                let name = rest
                    .first()
                    .ok_or_else(|| AppError::BadRequest("receita sem nome".into()))?;
                if rest.len() != 1 || !recipes.contains(name) {
                    return Err(AppError::BadRequest(format!(
                        "receita não declarada no manifesto: {}",
                        file.path
                    )));
                }
                let recipe = recipe_parser::parse(&text(file)?)?;
                validate_slug(&recipe.id, false)?;
            }
            "routines" => {
                let name = rest
                    .first()
                    .ok_or_else(|| AppError::BadRequest("rotina sem nome".into()))?;
                if rest.len() != 1 || !routines.contains(name) {
                    return Err(AppError::BadRequest(format!(
                        "rotina não declarada no manifesto: {}",
                        file.path
                    )));
                }
                let mut value: Value = serde_json::from_str(&text(file)?)
                    .map_err(|error| AppError::BadRequest(format!("rotina inválida: {error}")))?;
                value["workspace_id"] = Value::String("validation".into());
                value["is_active"] = Value::Bool(false);
                serde_json::from_value::<schedules::CreateScheduleInput>(value)
                    .map_err(|error| AppError::BadRequest(format!("rotina inválida: {error}")))?;
            }
            other => {
                return Err(AppError::BadRequest(format!(
                    "pasta não suportada no pacote: {other}"
                )));
            }
        }
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension(format!(
        "{}.{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("tmp"),
        uuid::Uuid::now_v7()
    ));
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, path)?;
    Ok(())
}

fn target_for(file: &PackFile, root: &Path) -> AppResult<(PathBuf, bool)> {
    let (kind, rest) = parts(&file.path)?;
    let joined = rest.join("/");
    match kind.as_str() {
        "agents" | "commands" => Ok((root.join(".opencode").join(&kind).join(&joined), false)),
        "skills" => Ok((root.join(".opencode/skills").join(&joined), false)),
        "recipes" => {
            let recipe = recipe_parser::parse(&text(file)?)?;
            validate_slug(&recipe.id, false)?;
            Ok((
                root.join(".opencode/recipes")
                    .join(format!("{}.yaml", recipe.id)),
                true,
            ))
        }
        "routines" => Ok((PathBuf::new(), false)),
        other => Err(AppError::BadRequest(format!(
            "pasta não suportada no pacote: {other}"
        ))),
    }
}

pub async fn import(
    state: &Arc<AppState>,
    workspace_id: &str,
    transfer: PackTransfer,
) -> AppResult<PackImportResult> {
    validate(&transfer)?;
    let workspace = workspaces::get(&state.db, workspace_id).await?;
    let root = Path::new(&workspace.root_path);
    let mut written = 0;
    let mut recipes_changed = false;
    for file in &transfer.files {
        if file.path.starts_with("routines/") {
            continue;
        }
        let (target, is_recipe) = target_for(file, root)?;
        let mut bytes = decode(file)?;
        if is_recipe && !target.exists() {
            let yaml = std::str::from_utf8(&bytes)
                .map_err(|_| AppError::BadRequest("receita não é UTF-8".into()))?
                .to_string();
            bytes = recipe_store::set_yaml_enabled(&yaml, false).into_bytes();
        }
        write_atomic(&target, &bytes)?;
        recipes_changed |= is_recipe;
        written += 1;
    }

    let mut imported_routines = 0;
    let existing = schedules::list(&state.db, workspace_id).await?;
    for file in &transfer.files {
        if !file.path.starts_with("routines/") {
            continue;
        }
        let mut value: Value = serde_json::from_str(&text(file)?)
            .map_err(|error| AppError::BadRequest(format!("rotina inválida: {error}")))?;
        let name = value
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if existing.iter().any(|schedule| schedule.name == name) {
            continue;
        }
        value["workspace_id"] = Value::String(workspace_id.to_string());
        value["is_active"] = Value::Bool(false);
        let input: schedules::CreateScheduleInput = serde_json::from_value(value)
            .map_err(|error| AppError::BadRequest(format!("rotina inválida: {error}")))?;
        schedules::create(&state.db, input).await?;
        imported_routines += 1;
    }

    if recipes_changed {
        recipe_store::sync_workspace(&state.db, workspace_id, root).await?;
        crate::api::recipes::sync_all_recipe_trigger_sources(state).await?;
    }
    Ok(PackImportResult {
        pack_id: transfer.manifest.id,
        written,
        routines: imported_routines,
    })
}
