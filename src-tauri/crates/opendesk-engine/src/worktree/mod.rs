//! Per-run workspace isolation.
//!
//! When the workspace is a Git repository, a run gets a detached worktree on a
//! temporary branch. Otherwise it gets a copy-on-write-style overlay directory.
//! Both modes support diff and merge behind an explicit approval flag.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path, process::Command};

mod git_ops;
mod manifest;
mod overlay;
mod sweeper;
#[cfg(test)]
include!("tests.rs");

pub use manifest::manifest_path;
pub use sweeper::cleanup;

use git_ops::{git_diff, git_output, has_head, is_git_repo, merge_git};
use manifest::{build_manifest, write_manifest};
use overlay::{copy_tree, merge_overlay, overlay_diff};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunWorktree {
    pub id: String,
    pub workspace_id: String,
    pub run_id: Option<String>,
    pub path: String,
    pub mode: String,
    pub branch: Option<String>,
    pub base_commit: Option<String>,
    pub status: String,
    pub created_at: String,
    pub merged_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct DiffStat {
    pub files: usize,
    pub insertions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MergeReport {
    pub id: String,
    pub mode: String,
    pub status: String,
    pub message: String,
}

pub fn prepare(
    workspace_id: &str,
    workspace_root: &Path,
    data_root: &Path,
    run_id: Option<&str>,
) -> AppResult<RunWorktree> {
    let id = uuid::Uuid::now_v7().to_string();
    let dir = data_root.join("worktrees").join(&id);
    if let Some(parent) = dir.parent() {
        fs::create_dir_all(parent)?;
    }

    let git = is_git_repo(workspace_root);
    let (mode, branch, base_commit) = if git && has_head(workspace_root) {
        // The last UUID segment is random; the timestamp prefix is shared by
        // worktrees created in the same millisecond.
        let suffix = id.rsplit('-').next().unwrap_or(&id);
        let branch = format!("opendesk/wt-{suffix}");
        let base = git_output(workspace_root, &["rev-parse", "HEAD"])
            .ok()
            .map(|value| value.trim().to_string());
        let output = Command::new("git")
            .arg("-C")
            .arg(workspace_root)
            .args(["worktree", "add", "-b", &branch])
            .arg(&dir)
            .arg("HEAD")
            .output()?;
        if !output.status.success() {
            return Err(AppError::Internal(format!(
                "git worktree add falhou: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        ("git", Some(branch), base)
    } else {
        copy_tree(workspace_root, &dir)?;
        // Baseline for conflict detection: without it the merge cannot tell a
        // file the run edited from one the user edited after the run started.
        let manifest = build_manifest(&dir)?;
        write_manifest(&dir, &manifest)?;
        ("overlay", None, None)
    };

    Ok(RunWorktree {
        id,
        workspace_id: workspace_id.to_string(),
        run_id: run_id.map(str::to_string),
        path: dir.to_string_lossy().to_string(),
        mode: mode.into(),
        branch,
        base_commit,
        status: "ACTIVE".into(),
        created_at: chrono::Utc::now().to_rfc3339(),
        merged_at: None,
    })
}

pub fn diff(worktree: &RunWorktree, workspace_root: &Path) -> AppResult<String> {
    match worktree.mode.as_str() {
        "git" => git_diff(Path::new(&worktree.path)),
        _ => overlay_diff(Path::new(&worktree.path), workspace_root),
    }
}

pub fn diff_stat(worktree: &RunWorktree, workspace_root: &Path) -> AppResult<DiffStat> {
    Ok(stat_from_diff(&diff(worktree, workspace_root)?))
}

pub fn stat_from_diff(diff: &str) -> DiffStat {
    let mut stat = DiffStat::default();
    if diff.trim() == "nenhuma alteração" {
        return stat;
    }
    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            stat.files += 1;
        } else if let Some(rest) = line.strip_prefix("A ") {
            if !rest.is_empty() {
                stat.files += 1;
                stat.insertions += 1;
            }
        } else if let Some(rest) = line.strip_prefix("M ") {
            if !rest.is_empty() {
                stat.files += 1;
            }
        } else if let Some(rest) = line.strip_prefix("D ") {
            if !rest.is_empty() {
                stat.files += 1;
                stat.deletions += 1;
            }
        } else if line.starts_with('+') && !line.starts_with("+++") {
            stat.insertions += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            stat.deletions += 1;
        }
    }
    stat
}

pub fn merge(
    worktree: &RunWorktree,
    workspace_root: &Path,
    approved: bool,
) -> AppResult<MergeReport> {
    if !approved {
        return Err(AppError::BadRequest(
            "merge de worktree exige aprovação explícita".into(),
        ));
    }
    if worktree.status != "ACTIVE" {
        return Err(AppError::BadRequest(format!(
            "worktree {} não está ativo ({})",
            worktree.id, worktree.status
        )));
    }
    match worktree.mode.as_str() {
        "git" => merge_git(worktree, workspace_root),
        _ => merge_overlay(worktree, workspace_root),
    }
}
