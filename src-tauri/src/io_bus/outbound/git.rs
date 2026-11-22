use crate::{
    db::io_bus::OutboundSink,
    error::{AppError, AppResult},
    io_bus::{IoContext, RunOutput, config_value, outbound::artifact_paths},
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default)]
    repo: Option<String>,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    push: bool,
    #[serde(default)]
    remote: Option<String>,
}

pub async fn run(
    _ctx: &IoContext,
    sink: &OutboundSink,
    output: &RunOutput,
) -> AppResult<Vec<String>> {
    let config: Config = config_value(sink)?;
    let repo = config
        .repo
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(&output.workspace_root));
    let repo = repo.canonicalize().unwrap_or(repo);
    let top = git(&repo, &["rev-parse", "--show-toplevel"]).await?;
    let top = PathBuf::from(top.trim());
    let branch = config
        .branch
        .clone()
        .unwrap_or_else(|| format!("opendesk/task-{}", short_id(output.task_id.as_deref())));
    let branch = sanitize_branch(&branch);
    let exists = git(&top, &["rev-parse", "--verify", &branch]).await.is_ok();
    if exists {
        git(&top, &["checkout", &branch]).await?;
    } else {
        git(&top, &["checkout", "-b", &branch]).await?;
    }

    let mut added = Vec::new();
    for artifact in artifact_paths(output) {
        let relative = artifact
            .strip_prefix(&top)
            .map_err(|_| {
                AppError::BadRequest(format!(
                    "artefato {} está fora do repositório {}",
                    artifact.display(),
                    top.display()
                ))
            })?
            .to_string_lossy()
            .to_string();
        git(&top, &["add", "--", &relative]).await?;
        added.push(relative);
    }
    if added.is_empty() {
        return Err(AppError::BadRequest(
            "GIT_COMMIT não encontrou artefatos dentro do repositório".into(),
        ));
    }
    let message = format!(
        "chore(agent): {} completed {} [run:{}]",
        output.artifacts.first().map(|_| "agent").unwrap_or("agent"),
        output.title,
        output.run_id
    );
    git(
        &top,
        &[
            "-c",
            "user.name=OpenDesk",
            "-c",
            "user.email=opendesk@localhost",
            "commit",
            "-m",
            &message,
        ],
    )
    .await?;
    if config.push {
        let remote = config.remote.as_deref().unwrap_or("origin");
        git(&top, &["push", remote, "HEAD"]).await?;
    }
    Ok(added)
}

async fn git(repo: &Path, args: &[&str]) -> AppResult<String> {
    let mut command = tokio::process::Command::new("git");
    command.args(args).current_dir(repo).kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let output = tokio::time::timeout(Duration::from_secs(60), command.output())
        .await
        .map_err(|_| AppError::Internal("git timeout".into()))?
        .map_err(|error| AppError::BadRequest(format!("git indisponível: {error}")))?;
    if !output.status.success() {
        return Err(AppError::Internal(format!(
            "git {} falhou: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn short_id(value: Option<&str>) -> String {
    value.unwrap_or("manual").chars().take(12).collect()
}

fn sanitize_branch(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '/' | '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn commits_artifacts_in_temp_repo() {
        let temp = tempfile::TempDir::new().expect("temp");
        let repo = temp.path();
        git(repo, &["init", "-q"]).await.expect("init");
        std::fs::write(repo.join("a.md"), "a").expect("a");
        git(repo, &["add", "a.md"]).await.expect("add");
        git(
            repo,
            &[
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@t",
                "commit",
                "-q",
                "-m",
                "init",
            ],
        )
        .await
        .expect("commit");
        std::fs::write(repo.join("b.md"), "b").expect("b");
        let out = RunOutput {
            workspace_id: "ws".into(),
            workspace_root: repo.to_string_lossy().to_string(),
            output_directory: None,
            task_id: Some("task-1".into()),
            run_id: "run-1".into(),
            title: "Tarefa".into(),
            summary: String::new(),
            artifacts: vec![crate::io_bus::RunArtifact {
                file_path: "b.md".into(),
                absolute_path: repo.join("b.md").to_string_lossy().to_string(),
                action_type: "CREATED".into(),
            }],
        };
        let ctx = crate::io_bus::IoContext {
            pool: crate::db::init(&root_db(repo)).await.expect("db"),
            paths: crate::paths::AppPaths::discover().expect("paths"),
        };
        let sink = crate::db::io_bus::OutboundSink {
            id: "s".into(),
            workspace_id: "ws".into(),
            name: "git".into(),
            sink_type: "GIT_COMMIT".into(),
            config_json: "{}".into(),
            is_enabled: true,
            last_run_at: None,
            last_status: None,
            last_error: None,
            created_at: "now".into(),
        };
        let written = run(&ctx, &sink, &out).await.expect("git run");
        assert_eq!(written, vec!["b.md"]);
    }

    fn root_db(repo: &Path) -> PathBuf {
        repo.join("test.db")
    }

    #[test]
    fn branch_is_sanitized() {
        assert_eq!(sanitize_branch("opendesk/task 1"), "opendesk/task-1");
    }
}
