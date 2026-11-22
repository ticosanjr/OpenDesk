use crate::error::{AppError, AppResult};
use std::{path::Path, process::Command};

use super::{MergeReport, RunWorktree};

pub(super) fn run_git(root: &Path, args: &[&str]) -> AppResult<std::process::Output> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(AppError::Internal(format!(
            "git {args:?} falhou: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output)
}

/// A merge commit would capture whatever is staged in the workspace. Refuse
/// when the index is dirty instead of committing someone else's work.
pub(super) fn ensure_no_staged_changes(workspace_root: &Path) -> AppResult<()> {
    let staged = git_output(workspace_root, &["diff", "--cached", "--name-only"])?;
    if !staged.trim().is_empty() {
        return Err(AppError::Conflict(format!(
            "workspace tem alterações no staging; faça commit ou stash antes de mesclar: {}",
            staged.lines().take(5).collect::<Vec<_>>().join(", ")
        )));
    }
    Ok(())
}

pub(super) fn merge_git(worktree: &RunWorktree, workspace_root: &Path) -> AppResult<MergeReport> {
    let path = Path::new(&worktree.path);
    let branch = worktree
        .branch
        .as_deref()
        .ok_or_else(|| AppError::Internal("worktree git sem branch".into()))?;

    ensure_no_staged_changes(workspace_root)?;

    // A run edits files without committing. Commit the worktree branch first so
    // the merge has something to bring back to the main tree.
    run_git(path, &["add", "-A"])?;
    let has_changes = !git_output(path, &["diff", "--cached", "--name-only"])?
        .trim()
        .is_empty();
    if !has_changes {
        return Ok(MergeReport {
            id: worktree.id.clone(),
            mode: worktree.mode.clone(),
            status: "UNCHANGED".into(),
            message: "nenhuma alteração para mesclar".into(),
        });
    }
    run_git(
        path,
        &[
            "-c",
            "user.name=OpenDesk",
            "-c",
            "user.email=opendesk@localhost",
            "commit",
            "--no-verify",
            "-m",
            "OpenDesk run changes",
        ],
    )?;

    // `--no-ff` without `--no-commit`: git builds the merge commit from the two
    // trees itself, so only the merge result enters it.
    let message = format!("Merge OpenDesk worktree {}", worktree.id);
    let merge = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .args([
            "-c",
            "user.name=OpenDesk",
            "-c",
            "user.email=opendesk@localhost",
        ])
        .args([
            "merge",
            "--no-ff",
            "--no-verify",
            "-m",
            message.as_str(),
            branch,
        ])
        .output()?;
    if !merge.status.success() {
        let _ = Command::new("git")
            .arg("-C")
            .arg(workspace_root)
            .args(["merge", "--abort"])
            .output();
        return Err(AppError::Conflict(format!(
            "conflito ao mesclar worktree {}: {}",
            worktree.id,
            String::from_utf8_lossy(&merge.stderr).trim()
        )));
    }
    Ok(MergeReport {
        id: worktree.id.clone(),
        mode: worktree.mode.clone(),
        status: "MERGED".into(),
        message: format!(
            "worktree mesclado no branch atual de {}",
            workspace_root.display()
        ),
    })
}

pub(super) fn git_diff(path: &Path) -> AppResult<String> {
    // Intent-to-add makes untracked files appear in `git diff` without staging
    // their content permanently.
    run_git(path, &["add", "-N", "."])?;
    git_output(path, &["diff", "--no-color"])
}

pub(super) fn is_git_repo(root: &Path) -> bool {
    git_output(root, &["rev-parse", "--is-inside-work-tree"])
        .map(|value| value.trim() == "true")
        .unwrap_or(false)
}

pub(super) fn has_head(root: &Path) -> bool {
    git_output(root, &["rev-parse", "--verify", "HEAD"]).is_ok()
}

pub(super) fn git_output(root: &Path, args: &[&str]) -> AppResult<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()?;
    if !output.status.success() {
        return Err(AppError::Internal(format!(
            "git {:?} falhou: {}",
            args,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
