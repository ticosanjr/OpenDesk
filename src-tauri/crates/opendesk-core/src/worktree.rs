//! Per-run workspace isolation.
//!
//! When the workspace is a Git repository, a run gets a detached worktree on a
//! temporary branch. Otherwise it gets a copy-on-write-style overlay directory.
//! Both modes support diff and merge; merge is only exposed behind an explicit
//! approval flag at the API/MCP layer.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use walkdir::WalkDir;

const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", "output", ".cache"];

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

pub fn cleanup(worktree: &RunWorktree, workspace_root: &Path) -> AppResult<()> {
    let path = Path::new(&worktree.path);
    if worktree.mode == "git" {
        let _ = Command::new("git")
            .arg("-C")
            .arg(workspace_root)
            .args(["worktree", "remove", "--force"])
            .arg(path)
            .output()?;
        if let Some(branch) = worktree.branch.as_deref() {
            let _ = Command::new("git")
                .arg("-C")
                .arg(workspace_root)
                .args(["branch", "-D", branch])
                .output()?;
        }
    } else if path.exists() {
        fs::remove_dir_all(path)?;
    }
    Ok(())
}

fn merge_git(worktree: &RunWorktree, workspace_root: &Path) -> AppResult<MergeReport> {
    let path = Path::new(&worktree.path);
    let branch = worktree
        .branch
        .as_deref()
        .ok_or_else(|| AppError::Internal("worktree git sem branch".into()))?;

    // A run edits files without committing. Commit the worktree branch first so
    // the merge has something to bring back to the main tree.
    let _ = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["add", "-A"])
        .output()?;
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
    let commit = Command::new("git")
        .arg("-C")
        .arg(path)
        .args([
            "-c",
            "user.name=OpenDesk",
            "-c",
            "user.email=opendesk@localhost",
        ])
        .args(["commit", "-m", "OpenDesk run changes"])
        .output()?;
    if !commit.status.success() {
        return Err(AppError::Internal(format!(
            "commit do worktree falhou: {}",
            String::from_utf8_lossy(&commit.stderr).trim()
        )));
    }

    let merge = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .args([
            "-c",
            "user.name=OpenDesk",
            "-c",
            "user.email=opendesk@localhost",
        ])
        .args(["merge", "--no-ff", "--no-commit", branch])
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
    let commit = Command::new("git")
        .arg("-C")
        .arg(workspace_root)
        .args([
            "-c",
            "user.name=OpenDesk",
            "-c",
            "user.email=opendesk@localhost",
        ])
        .args([
            "commit",
            "-m",
            &format!("Merge OpenDesk worktree {}", worktree.id),
        ])
        .output()?;
    if !commit.status.success() {
        let _ = Command::new("git")
            .arg("-C")
            .arg(workspace_root)
            .args(["merge", "--abort"])
            .output();
        return Err(AppError::Internal(format!(
            "commit do merge falhou: {}",
            String::from_utf8_lossy(&commit.stderr).trim()
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

fn merge_overlay(worktree: &RunWorktree, workspace_root: &Path) -> AppResult<MergeReport> {
    let source = Path::new(&worktree.path);
    let mut copied = 0usize;
    for entry in WalkDir::new(source).into_iter().filter_entry(not_skipped) {
        let entry = entry.map_err(|error| AppError::Internal(error.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        let target = workspace_root.join(relative);
        let changed = match fs::read(&target) {
            Ok(existing) => existing != fs::read(entry.path())?,
            Err(_) => true,
        };
        if !changed {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(entry.path(), &target)?;
        copied += 1;
    }
    Ok(MergeReport {
        id: worktree.id.clone(),
        mode: worktree.mode.clone(),
        status: if copied == 0 { "UNCHANGED" } else { "MERGED" }.into(),
        message: format!("{copied} arquivo(s) aplicado(s) no workspace"),
    })
}

fn git_diff(path: &Path) -> AppResult<String> {
    // Intent-to-add makes untracked files appear in `git diff` without staging
    // their content permanently.
    let _ = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["add", "-N", "."])
        .output()?;
    git_output(path, &["diff", "--no-color"])
}

fn overlay_diff(worktree: &Path, workspace_root: &Path) -> AppResult<String> {
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
        let target = workspace_root.join(relative);
        if !target.exists() {
            lines.push(format!("A {}", relative.display()));
        } else if fs::read(&target)? != fs::read(entry.path())? {
            lines.push(format!("M {}", relative.display()));
        }
    }
    if lines.is_empty() {
        lines.push("nenhuma alteração".into());
    }
    Ok(lines.join("\n"))
}

fn copy_tree(source: &Path, target: &Path) -> AppResult<()> {
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

fn not_skipped(entry: &walkdir::DirEntry) -> bool {
    if entry.depth() == 0 {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    !SKIP_DIRS.contains(&name.as_ref())
}

fn is_git_repo(root: &Path) -> bool {
    git_output(root, &["rev-parse", "--is-inside-work-tree"])
        .map(|value| value.trim() == "true")
        .unwrap_or(false)
}

fn has_head(root: &Path) -> bool {
    git_output(root, &["rev-parse", "--verify", "HEAD"]).is_ok()
}

fn git_output(root: &Path, args: &[&str]) -> AppResult<String> {
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

#[allow(dead_code)]
fn path_of(worktree: &RunWorktree) -> PathBuf {
    PathBuf::from(&worktree.path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(root: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["-c", "user.name=Test", "-c", "user.email=test@localhost"])
            .args(args)
            .output()
            .expect("git");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn git_worktree_diff_and_merge_with_approval() {
        let dir = tempfile::tempdir().expect("temp");
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).expect("ws");
        git(&workspace, &["init"]);
        fs::write(workspace.join("file.txt"), "base\n").expect("file");
        git(&workspace, &["add", "file.txt"]);
        git(&workspace, &["commit", "-m", "base"]);

        let worktree = prepare("ws-1", &workspace, dir.path(), Some("run-1")).expect("prepare");
        assert_eq!(worktree.mode, "git");
        let path = Path::new(&worktree.path);
        fs::write(path.join("file.txt"), "base\nchange\n").expect("edit");
        fs::write(path.join("new.txt"), "new\n").expect("new");

        let diff = diff(&worktree, &workspace).expect("diff");
        assert!(diff.contains("+change"), "{diff}");
        assert!(diff.contains("new.txt"), "{diff}");
        assert!(merge(&worktree, &workspace, false).is_err());

        let report = merge(&worktree, &workspace, true).expect("merge");
        assert_eq!(report.status, "MERGED");
        assert_eq!(
            fs::read_to_string(workspace.join("file.txt")).expect("merged"),
            "base\nchange\n"
        );
        assert!(workspace.join("new.txt").exists());
        cleanup(&worktree, &workspace).expect("cleanup");
        assert!(!path.exists());
    }

    #[test]
    fn overlay_mode_copies_and_applies_without_git() {
        let dir = tempfile::tempdir().expect("temp");
        let workspace = dir.path().join("ws");
        fs::create_dir_all(workspace.join("input")).expect("ws");
        fs::write(workspace.join("input/a.txt"), "base\n").expect("file");

        let worktree = prepare("ws-2", &workspace, dir.path(), None).expect("prepare");
        assert_eq!(worktree.mode, "overlay");
        fs::write(
            Path::new(&worktree.path).join("input/a.txt"),
            "base\nchanged\n",
        )
        .expect("edit");
        let diff = diff(&worktree, &workspace).expect("diff");
        assert!(diff.contains("input/a.txt"), "{diff}");

        let report = merge(&worktree, &workspace, true).expect("merge");
        assert_eq!(report.status, "MERGED");
        assert_eq!(
            fs::read_to_string(workspace.join("input/a.txt")).expect("merged"),
            "base\nchanged\n"
        );
        cleanup(&worktree, &workspace).expect("cleanup");
    }
}
