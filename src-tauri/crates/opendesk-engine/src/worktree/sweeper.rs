use crate::error::{AppError, AppResult};
use std::{fs, path::Path, process::Command};

use super::git_ops::{git_output, run_git};
use super::{RunWorktree, manifest_path};

pub fn cleanup(worktree: &RunWorktree, workspace_root: &Path) -> AppResult<()> {
    let path = Path::new(&worktree.path);
    let sidecar = manifest_path(path);
    if worktree.mode == "git" {
        let path_text = path.to_string_lossy().to_string();
        let removed = run_git(
            workspace_root,
            &["worktree", "remove", "--force", path_text.as_str()],
        );
        if let Err(error) = removed {
            // The directory may already be gone while the administrative entry
            // lingers; prune before deciding that cleanup failed.
            let _ = Command::new("git")
                .arg("-C")
                .arg(workspace_root)
                .args(["worktree", "prune"])
                .output();
            if path.exists() {
                return Err(AppError::Internal(format!(
                    "não foi possível remover o worktree {}: {error}",
                    path.display()
                )));
            }
        }
        if let Some(branch) = worktree.branch.as_deref() {
            let listed =
                git_output(workspace_root, &["branch", "--list", branch]).unwrap_or_default();
            if !listed.trim().is_empty()
                && let Err(error) = run_git(workspace_root, &["branch", "-D", branch])
            {
                return Err(AppError::Internal(format!(
                    "não foi possível remover o branch {branch}: {error}"
                )));
            }
        }
    } else if path.exists() {
        fs::remove_dir_all(path)?;
    }
    if sidecar.exists() {
        fs::remove_file(&sidecar)?;
    }
    Ok(())
}
