use crate::{
    config_sync::{file_ops::validate_slug, markdown},
    db::schedules,
    error::{AppError, AppResult},
    recipes::{parser as recipe_parser, store as recipe_store},
};
use include_dir::Dir;

use super::{BUNDLE, PackManifest, collect_dir, file_name, safe_relative, sha256};

#[derive(Debug, Clone)]
pub(super) struct Planned {
    pub(super) target: String,
    pub(super) kind: String,
    pub(super) bytes: Vec<u8>,
    pub(super) sha256: String,
    pub(super) recipe_id: Option<String>,
}

pub(super) struct LoadedPack {
    pub(super) manifest: PackManifest,
    pub(super) planned: Vec<Planned>,
    pub(super) routines: Vec<(String, schedules::CreateScheduleInput)>,
}

pub(super) fn planned_for(
    manifest: &PackManifest,
    _dir: &'static Dir<'static>,
) -> AppResult<Vec<Planned>> {
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
        let text = std::str::from_utf8(file.contents())
            .map_err(|_| AppError::BadRequest(format!("comando não é UTF-8: {name}")))?;
        let document = markdown::parse(text)?;
        if document.body.trim().is_empty() {
            return Err(AppError::BadRequest(format!(
                "comando sem instruções: {name}"
            )));
        }
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
            if relative == "SKILL.md" {
                let text = std::str::from_utf8(bytes)
                    .map_err(|_| AppError::BadRequest(format!("skill não é UTF-8: {id}")))?;
                let document = markdown::parse(text)?;
                if markdown::get_string(&document.frontmatter, "description")
                    .unwrap_or_default()
                    .trim()
                    .is_empty()
                {
                    return Err(AppError::BadRequest(format!(
                        "skill sem description no frontmatter: {id}"
                    )));
                }
                if document.body.trim().is_empty() {
                    return Err(AppError::BadRequest(format!("skill sem conteúdo: {id}")));
                }
            }
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
        // Pack recipes are installed disabled, regardless of the YAML in the
        // bundle. Enabling a recipe is always an explicit, post-review action.
        let disabled = recipe_store::set_yaml_enabled(text, false);
        push(
            format!(".opencode/recipes/{id}.yaml"),
            "recipe",
            disabled.as_bytes(),
            Some(id),
        );
    }
    for folder in &manifest.contents.folders {
        safe_relative(folder)?;
    }
    Ok(planned)
}

pub(super) fn planned_routines(
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

pub(super) fn load_pack(
    manifest: PackManifest,
    dir: &'static Dir<'static>,
    workspace_id: &str,
) -> AppResult<LoadedPack> {
    let planned = planned_for(&manifest, dir)?;
    let routines = planned_routines(&manifest, dir, workspace_id)?;
    let recipe_ids: std::collections::HashSet<&str> = planned
        .iter()
        .filter_map(|file| file.recipe_id.as_deref())
        .collect();
    for (file, input) in &routines {
        if input.name.trim().is_empty() {
            return Err(AppError::BadRequest(format!("rotina {file} sem nome")));
        }
        if !schedules::TRIGGER_TYPES.contains(&input.trigger_type.as_str()) {
            return Err(AppError::BadRequest(format!(
                "rotina {file} com trigger_type inválido '{}'",
                input.trigger_type
            )));
        }
        if input.trigger_type == "CRON" && input.cron_expression.trim().is_empty() {
            return Err(AppError::BadRequest(format!(
                "rotina {file} CRON sem cron_expression"
            )));
        }
        if input.target_kind == "RECIPE"
            && input
                .recipe_id
                .as_deref()
                .map(|recipe_id| !recipe_ids.contains(recipe_id))
                .unwrap_or(true)
        {
            return Err(AppError::BadRequest(format!(
                "rotina {file} aponta para receita fora do pacote"
            )));
        }
    }
    Ok(LoadedPack {
        manifest,
        planned,
        routines,
    })
}
