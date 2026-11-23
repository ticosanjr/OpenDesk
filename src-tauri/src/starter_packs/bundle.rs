//! Embedded bundle access: catalog, manifests, path validation and checksums.

use std::path::{Component, Path, PathBuf};

use include_dir::Dir;
use sha2::{Digest, Sha256};

use crate::{
    config_sync::paths::StudioPaths,
    error::{AppError, AppResult},
};

use super::{
    BUNDLE,
    plan::{planned_for, planned_routines},
    types::{CatalogFile, PackManifest},
};

pub(super) fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn catalog() -> AppResult<CatalogFile> {
    let file = BUNDLE
        .get_file("catalog.json")
        .ok_or_else(|| AppError::Internal("starter-packs/catalog.json ausente".into()))?;
    serde_json::from_slice(file.contents())
        .map_err(|error| AppError::Internal(format!("catalog.json inválido: {error}")))
}

pub(super) fn pack_dir(id: &str) -> AppResult<&'static Dir<'static>> {
    BUNDLE
        .get_dir(format!("packs/{id}"))
        .ok_or_else(|| AppError::NotFound(format!("pacote {id}")))
}

pub(super) fn load_manifest(id: &str) -> AppResult<(PackManifest, &'static Dir<'static>)> {
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
