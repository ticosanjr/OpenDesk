use crate::error::{AppError, AppResult};
use std::{
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

use super::manifest::{hash_file, load_manifest, not_skipped, relative_key};
use super::{MergeReport, RunWorktree};

pub(super) fn merge_overlay(
    worktree: &RunWorktree,
    workspace_root: &Path,
) -> AppResult<MergeReport> {
    let source = Path::new(&worktree.path);
    let manifest = load_manifest(source)?.ok_or_else(|| {
        AppError::Conflict(format!(
            "worktree {} não tem manifesto base; descarte-o e crie uma nova execução",
            worktree.id
        ))
    })?;

    let mut copies: Vec<PathBuf> = Vec::new();
    let mut removals: Vec<String> = Vec::new();
    let mut conflicts: Vec<String> = Vec::new();

    // Phase 1: classify. Nothing is written until every file is understood.
    for entry in WalkDir::new(source).into_iter().filter_entry(not_skipped) {
        let entry = entry.map_err(|error| AppError::Internal(error.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let key = relative_key(relative);
        let run_hash = hash_file(entry.path())?;
        let base = manifest.files.get(&key);
        if base == Some(&run_hash) {
            // The run never touched this file; keep whatever the user has.
            continue;
        }
        let target = workspace_root.join(relative);
        match fs::metadata(&target) {
            Ok(meta) if meta.is_file() => {
                let current = hash_file(&target)?;
                if current == run_hash {
                    continue;
                }
                if base.is_some_and(|hash| *hash == current) {
                    copies.push(relative.to_path_buf());
                } else {
                    conflicts.push(key);
                }
            }
            Ok(_) => conflicts.push(key),
            Err(_) => copies.push(relative.to_path_buf()),
        }
    }

    // Files present in the baseline and gone from the worktree were deleted by
    // the run. Apply only when the workspace still matches the baseline.
    for (key, base_hash) in &manifest.files {
        if source.join(key).exists() {
            continue;
        }
        let target = workspace_root.join(key);
        let Ok(meta) = fs::metadata(&target) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        if hash_file(&target)? == *base_hash {
            removals.push(key.clone());
        } else {
            conflicts.push(key.clone());
        }
    }

    if !conflicts.is_empty() {
        conflicts.sort();
        return Err(AppError::Conflict(format!(
            "conflito ao mesclar worktree {}: {} arquivo(s) alterado(s) nos dois lados: {}",
            worktree.id,
            conflicts.len(),
            conflicts.join(", ")
        )));
    }

    // Phase 2: apply.
    let mut copied = 0usize;
    for relative in copies {
        let target = workspace_root.join(&relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source.join(&relative), &target)?;
        copied += 1;
    }
    for key in &removals {
        fs::remove_file(workspace_root.join(key))?;
    }
    let status = if copied == 0 && removals.is_empty() {
        "UNCHANGED"
    } else {
        "MERGED"
    };
    Ok(MergeReport {
        id: worktree.id.clone(),
        mode: worktree.mode.clone(),
        status: status.into(),
        message: format!(
            "{copied} arquivo(s) aplicado(s) e {} removido(s) no workspace",
            removals.len()
        ),
    })
}

pub(super) fn overlay_diff(worktree: &Path, workspace_root: &Path) -> AppResult<String> {
    let manifest = load_manifest(worktree)?;
    let mut lines = Vec::new();
    for entry in WalkDir::new(worktree).into_iter().filter_entry(not_skipped) {
        let entry = entry.map_err(|error| AppError::Internal(error.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(worktree)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let key = relative_key(relative);
        let target = workspace_root.join(relative);
        let run_hash = hash_file(entry.path())?;
        if manifest
            .as_ref()
            .and_then(|manifest| manifest.files.get(&key))
            == Some(&run_hash)
        {
            // Untouched by the run: any difference belongs to the user.
            continue;
        }
        if !target.exists() {
            lines.push(format!("A {key}"));
        } else if fs::read(&target)? != fs::read(entry.path())? {
            lines.push(format!("M {key}"));
        }
    }
    if let Some(manifest) = &manifest {
        for key in manifest.files.keys() {
            if !worktree.join(key).exists() && workspace_root.join(key).exists() {
                lines.push(format!("D {key}"));
            }
        }
    }
    if lines.is_empty() {
        lines.push("nenhuma alteração".into());
    }
    Ok(lines.join("\n"))
}

pub(super) fn copy_tree(source: &Path, target: &Path) -> AppResult<()> {
    for entry in WalkDir::new(source).into_iter().filter_entry(not_skipped) {
        let entry = entry.map_err(|error| AppError::Internal(error.to_string()))?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let destination = target.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination)?;
        } else if entry.file_type().is_file() {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(entry.path(), &destination)?;
        }
    }
    Ok(())
}
