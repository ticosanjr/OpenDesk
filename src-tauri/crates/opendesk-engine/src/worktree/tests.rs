#[cfg(test)]
mod tests {
    use super::*;
    use super::git_ops::git_output;

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
