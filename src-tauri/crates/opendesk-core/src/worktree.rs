//! Per-run workspace isolation.
//!
//! When the workspace is a Git repository, a run gets a detached worktree on a
//! temporary branch. Otherwise it gets a copy-on-write-style overlay directory.
//! Both modes support diff and merge; merge is only exposed behind an explicit
//! approval flag at the API/MCP layer.

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufReader, Read},
    path::{Path, PathBuf},
    process::Command,
};
use walkdir::WalkDir;

const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", "output", ".cache"];

/// Baseline of an overlay worktree: sha256 per file at creation time. The merge
/// uses it to tell apart files the run changed from files the user changed
/// after the run started.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct BaseManifest {
    version: u32,
    files: BTreeMap<String, String>,
}

impl Default for BaseManifest {
    fn default() -> Self {
        Self {
            version: 1,
            files: BTreeMap::new(),
        }
    }
}

pub(crate) fn manifest_path(worktree_path: &Path) -> PathBuf {
    let mut name = worktree_path.as_os_str().to_owned();
    name.push(".manifest.json");
    PathBuf::from(name)
}

fn relative_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn hash_file(path: &Path) -> AppResult<String> {
    let mut hasher = Sha256::new();
    let mut reader = BufReader::new(fs::File::open(path)?);
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn build_manifest(root: &Path) -> AppResult<BaseManifest> {
    let mut manifest = BaseManifest::default();
    for entry in WalkDir::new(root).into_iter().filter_entry(not_skipped) {
        let entry = entry.map_err(|error| AppError::Internal(error.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| AppError::Internal(error.to_string()))?;
        manifest
            .files
            .insert(relative_key(relative), hash_file(entry.path())?);
    }
    Ok(manifest)
}

fn write_manifest(worktree_path: &Path, manifest: &BaseManifest) -> AppResult<()> {
    let path = manifest_path(worktree_path);
    let body = serde_json::to_vec(manifest)
        .map_err(|error| AppError::Internal(format!("manifesto do worktree: {error}")))?;
    fs::write(&path, body)?;
    Ok(())
}

fn load_manifest(worktree_path: &Path) -> AppResult<Option<BaseManifest>> {
    let path = manifest_path(worktree_path);
    let body = match fs::read(&path) {
        Ok(body) => body,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let manifest = serde_json::from_slice(&body)
        .map_err(|error| AppError::Internal(format!("manifesto do worktree inválido: {error}")))?;
    Ok(Some(manifest))
}

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

fn run_git(root: &Path, args: &[&str]) -> AppResult<std::process::Output> {
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
fn ensure_no_staged_changes(workspace_root: &Path) -> AppResult<()> {
    let staged = git_output(workspace_root, &["diff", "--cached", "--name-only"])?;
    if !staged.trim().is_empty() {
        return Err(AppError::Conflict(format!(
            "workspace tem alterações no staging; faça commit ou stash antes de mesclar: {}",
            staged.lines().take(5).collect::<Vec<_>>().join(", ")
        )));
    }
    Ok(())
}

fn merge_git(worktree: &RunWorktree, workspace_root: &Path) -> AppResult<MergeReport> {
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

fn merge_overlay(worktree: &RunWorktree, workspace_root: &Path) -> AppResult<MergeReport> {
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

fn git_diff(path: &Path) -> AppResult<String> {
    // Intent-to-add makes untracked files appear in `git diff` without staging
    // their content permanently.
    run_git(path, &["add", "-N", "."])?;
    git_output(path, &["diff", "--no-color"])
}

fn overlay_diff(worktree: &Path, workspace_root: &Path) -> AppResult<String> {
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
    fn overlay_merge_detects_conflicts_without_overwriting() {
        let dir = tempfile::tempdir().expect("temp");
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).expect("ws");
        fs::write(workspace.join("a.txt"), "base\n").expect("a");
        fs::write(workspace.join("b.txt"), "b\n").expect("b");
        fs::write(workspace.join("c.txt"), "c\n").expect("c");

        let worktree = prepare("ws-1", &workspace, dir.path(), None).expect("prepare");
        let path = Path::new(&worktree.path);
        // The run edits a, creates d and deletes b. It never touches c.
        fs::write(path.join("a.txt"), "run-a\n").expect("run a");
        fs::write(path.join("d.txt"), "run-d\n").expect("run d");
        fs::remove_file(path.join("b.txt")).expect("run delete b");

        // The user edits a (same file as the run) and c (untouched by the run).
        fs::write(workspace.join("a.txt"), "user-a\n").expect("user a");
        fs::write(workspace.join("c.txt"), "user-c\n").expect("user c");

        let diff = diff(&worktree, &workspace).expect("diff");
        assert!(diff.contains("M a.txt"), "{diff}");
        assert!(
            !diff.contains("c.txt"),
            "edição do usuário não é da run: {diff}"
        );
        assert!(diff.contains("D b.txt"), "{diff}");
        assert!(diff.contains("A d.txt"), "{diff}");

        let error = merge(&worktree, &workspace, true).expect_err("conflict");
        assert!(error.to_string().contains("a.txt"), "{error}");
        assert_eq!(
            fs::read_to_string(workspace.join("a.txt")).expect("a"),
            "user-a\n"
        );
        assert_eq!(
            fs::read_to_string(workspace.join("b.txt")).expect("b"),
            "b\n"
        );
        assert_eq!(
            fs::read_to_string(workspace.join("c.txt")).expect("c"),
            "user-c\n"
        );
        assert!(
            !workspace.join("d.txt").exists(),
            "conflito não aplica nada"
        );

        fs::write(workspace.join("a.txt"), "base\n").expect("reset a");
        let report = merge(&worktree, &workspace, true).expect("merge");
        assert_eq!(report.status, "MERGED");
        assert_eq!(
            fs::read_to_string(workspace.join("a.txt")).expect("a"),
            "run-a\n"
        );
        assert_eq!(
            fs::read_to_string(workspace.join("c.txt")).expect("c"),
            "user-c\n"
        );
        assert!(!workspace.join("b.txt").exists());
        assert_eq!(
            fs::read_to_string(workspace.join("d.txt")).expect("d"),
            "run-d\n"
        );
    }

    #[test]
    fn overlay_merge_requires_a_baseline_manifest() {
        let dir = tempfile::tempdir().expect("temp");
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).expect("ws");
        fs::write(workspace.join("a.txt"), "base\n").expect("a");

        let worktree = prepare("ws-2", &workspace, dir.path(), None).expect("prepare");
        fs::write(Path::new(&worktree.path).join("a.txt"), "run\n").expect("run");
        fs::remove_file(manifest_path(Path::new(&worktree.path))).expect("manifest");
        let error = merge(&worktree, &workspace, true).expect_err("manifest");
        assert!(error.to_string().contains("manifesto"), "{error}");
        assert_eq!(
            fs::read_to_string(workspace.join("a.txt")).expect("a"),
            "base\n"
        );
    }

    #[test]
    fn git_merge_refuses_foreign_staged_changes() {
        let dir = tempfile::tempdir().expect("temp");
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).expect("ws");
        git(&workspace, &["init"]);
        fs::write(workspace.join("file.txt"), "base\n").expect("file");
        git(&workspace, &["add", "file.txt"]);
        git(&workspace, &["commit", "-m", "base"]);

        let worktree = prepare("ws-1", &workspace, dir.path(), Some("run-1")).expect("prepare");
        fs::write(Path::new(&worktree.path).join("file.txt"), "run\n").expect("edit");

        fs::write(workspace.join("other.txt"), "staged\n").expect("other");
        git(&workspace, &["add", "other.txt"]);
        let before = git_output(&workspace, &["rev-list", "--count", "HEAD"]).expect("count");

        let error = merge(&worktree, &workspace, true).expect_err("staged");
        assert!(error.to_string().contains("staging"), "{error}");
        let after = git_output(&workspace, &["rev-list", "--count", "HEAD"]).expect("count");
        assert_eq!(before, after, "nenhum commit de merge pode ser criado");
        assert_eq!(
            fs::read_to_string(workspace.join("file.txt")).expect("file"),
            "base\n"
        );
    }

    #[test]
    fn git_cleanup_removes_the_branch_and_stays_idempotent() {
        let dir = tempfile::tempdir().expect("temp");
        let workspace = dir.path().join("ws");
        fs::create_dir_all(&workspace).expect("ws");
        git(&workspace, &["init"]);
        fs::write(workspace.join("file.txt"), "base\n").expect("file");
        git(&workspace, &["add", "file.txt"]);
        git(&workspace, &["commit", "-m", "base"]);

        let worktree = prepare("ws-1", &workspace, dir.path(), None).expect("prepare");
        let branch = worktree.branch.clone().expect("branch");
        cleanup(&worktree, &workspace).expect("cleanup");
        assert!(!Path::new(&worktree.path).exists());
        let listed = git_output(&workspace, &["branch", "--list", &branch]).expect("branch");
        assert!(listed.trim().is_empty(), "{listed}");
        cleanup(&worktree, &workspace).expect("cleanup idempotente");
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
