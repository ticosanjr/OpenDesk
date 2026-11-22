use crate::runtime::RuntimeFileDiff;

use std::time::Duration;
use tokio::process::Command;

/// Collects files changed during a CLI run so artifacts/provenance work
/// without an OpenCode diff endpoint. Uses `git status --porcelain`.
pub async fn git_diffs(workspace_root: &str) -> Vec<RuntimeFileDiff> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(workspace_root)
        .args(["status", "--porcelain"])
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let output = tokio::time::timeout(Duration::from_secs(15), command.output()).await;
    let Ok(Ok(output)) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            if line.len() < 4 {
                return None;
            }
            let code = line[..2].trim();
            let path = line[3..].trim().trim_matches('"').to_string();
            if path.is_empty() {
                return None;
            }
            let status = if code.contains('A') || code == "??" {
                "added"
            } else if code.contains('D') {
                "deleted"
            } else {
                "modified"
            };
            Some(RuntimeFileDiff {
                path,
                status: status.into(),
                patch: String::new(),
                additions: 0,
                deletions: 0,
            })
        })
        .take(50)
        .collect()
}
