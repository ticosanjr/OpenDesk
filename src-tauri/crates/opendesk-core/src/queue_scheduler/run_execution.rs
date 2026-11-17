use crate::{
    db::{
        artifacts::{self, CreateArtifactInput},
        queue, tasks,
    },
    events::{ArtifactProvenanceDto, EventBus, OpenDeskWsEvent},
    opencode_client::session_runner::RunOutcome,
    paths::AppPaths,
};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use walkdir::WalkDir;

/// Everything that can be derived before the OpenCode session starts.
#[derive(Debug, Clone)]
pub struct RunPlan {
    pub run_id: String,
    pub log_path: PathBuf,
    pub phase: String,
    pub title: String,
    pub output_dir: PathBuf,
}

pub fn plan_run(
    job: &queue::QueueJob,
    task: Option<&tasks::Task>,
    paths: &AppPaths,
    workspace_root: &Path,
) -> RunPlan {
    let run_id = Uuid::now_v7().to_string();
    let log_path = paths.run_logs_dir.join(format!("{run_id}.jsonl"));
    let phase = if job.job_kind == "audit" {
        "audit"
    } else {
        job.resolved_mode.as_str()
    }
    .to_string();
    let title = task.map(|task| task.title.clone()).unwrap_or_else(|| {
        job.task_title
            .clone()
            .unwrap_or_else(|| "Scheduled task".into())
    });
    let output_dir = job
        .output_dir
        .clone()
        .map(PathBuf::from)
        .or_else(|| {
            task.and_then(|task| task.output_dir_path.clone())
                .map(PathBuf::from)
        })
        .unwrap_or_else(|| {
            workspace_root
                .join("output")
                .join(chrono::Utc::now().format("%Y-%m").to_string())
                .join(crate::dsh_router::slugify(&title))
        });

    RunPlan {
        run_id,
        log_path,
        phase,
        title,
        output_dir,
    }
}

/// Records every diff/write reported by a run. Kept separate from queue
/// orchestration so provenance can be tested without an OpenCode session.
#[allow(clippy::too_many_arguments)]
pub async fn record_artifacts(
    pool: &SqlitePool,
    events: &EventBus,
    job: &queue::QueueJob,
    run_id: &str,
    reason: &str,
    workspace_root: &Path,
    output_dir: &Path,
    outcome: &RunOutcome,
) {
    let mut seen_files = std::collections::HashSet::new();
    for diff in &outcome.diffs {
        seen_files.insert(diff.file.clone());
        let action = match diff.status.as_str() {
            "added" => "CREATED",
            "deleted" => "DELETED",
            _ => "MODIFIED",
        };
        record_artifact(
            pool,
            events,
            &job.workspace_id,
            job.task_id.as_deref(),
            run_id,
            &job.resolved_agent,
            reason,
            workspace_root,
            &diff.file,
            action,
            Some(&diff.patch),
            None,
        )
        .await;
    }

    for write in &outcome.writes {
        if seen_files.contains(&write.path) {
            continue;
        }
        record_artifact(
            pool,
            events,
            &job.workspace_id,
            job.task_id.as_deref(),
            run_id,
            &job.resolved_agent,
            reason,
            workspace_root,
            &write.path,
            &write.action,
            write.diff_patch.as_deref(),
            None,
        )
        .await;
    }

    scan_output_directory(
        pool,
        events,
        job,
        run_id,
        reason,
        workspace_root,
        output_dir,
    )
    .await;
}

#[allow(clippy::too_many_arguments)]
async fn scan_output_directory(
    pool: &SqlitePool,
    events: &EventBus,
    job: &queue::QueueJob,
    run_id: &str,
    reason: &str,
    workspace_root: &Path,
    output_dir: &Path,
) {
    if !output_dir.is_dir() {
        return;
    }
    let mut recorded = 0;
    for entry in WalkDir::new(output_dir)
        .max_depth(8)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if recorded >= 200 {
            break;
        }
        let absolute = entry.path();
        let relative = absolute
            .strip_prefix(workspace_root)
            .unwrap_or(absolute)
            .to_string_lossy()
            .replace('\\', "/");
        if artifacts::exists(pool, run_id, &relative)
            .await
            .unwrap_or(false)
        {
            continue;
        }
        let (size, hash) = file_fingerprint(absolute);
        let previous = artifacts::latest_for_path(pool, &job.workspace_id, &relative)
            .await
            .ok()
            .flatten();
        if previous.as_ref().is_some_and(|artifact| {
            artifact.action_type != "DELETED"
                && artifact.file_size_bytes == size
                && artifact.content_sha256.as_deref() == hash.as_deref()
        }) {
            continue;
        }
        recorded += 1;
        let action = if previous.is_some() {
            "MODIFIED"
        } else {
            "CREATED"
        };
        record_artifact(
            pool,
            events,
            &job.workspace_id,
            job.task_id.as_deref(),
            run_id,
            &job.resolved_agent,
            reason,
            workspace_root,
            &relative,
            action,
            None,
            previous
                .as_ref()
                .and_then(|artifact| artifact.content_sha256.as_deref()),
        )
        .await;
    }
}

/// Artifacts for a chat turn that used tools. The run is already persisted with
/// source=chat and the channel in origin metadata, so the artifact list can
/// derive the badge by joining task_runs.
#[allow(clippy::too_many_arguments)]
pub async fn record_chat_artifacts(
    pool: &SqlitePool,
    events: &EventBus,
    workspace_id: &str,
    run_id: &str,
    agent: &str,
    reason: &str,
    workspace_root: &Path,
    diffs: &[crate::opencode_client::rest_client::FileDiff],
    writes: &[crate::opencode_client::session_runner::FileWrite],
) {
    let mut seen = std::collections::HashSet::new();
    for diff in diffs {
        seen.insert(diff.file.clone());
        let action = match diff.status.as_str() {
            "added" => "CREATED",
            "deleted" => "DELETED",
            _ => "MODIFIED",
        };
        record_artifact(
            pool,
            events,
            workspace_id,
            None,
            run_id,
            agent,
            reason,
            workspace_root,
            &diff.file,
            action,
            Some(&diff.patch),
            None,
        )
        .await;
    }
    for write in writes {
        if seen.contains(&write.path) {
            continue;
        }
        record_artifact(
            pool,
            events,
            workspace_id,
            None,
            run_id,
            agent,
            reason,
            workspace_root,
            &write.path,
            &write.action,
            write.diff_patch.as_deref(),
            None,
        )
        .await;
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn record_artifact(
    pool: &SqlitePool,
    events: &EventBus,
    workspace_id: &str,
    task_id: Option<&str>,
    run_id: &str,
    agent: &str,
    reason: &str,
    workspace_root: &Path,
    file_path: &str,
    action: &str,
    diff_patch: Option<&str>,
    previous_sha256: Option<&str>,
) {
    if artifacts::exists(pool, run_id, file_path)
        .await
        .unwrap_or(false)
    {
        return;
    }

    let absolute = if Path::new(file_path).is_absolute() {
        PathBuf::from(file_path)
    } else {
        workspace_root.join(file_path)
    };
    let relative = absolute
        .strip_prefix(workspace_root)
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| file_path.to_string());
    let file_type = absolute
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_string();
    let (size, hash) = file_fingerprint(&absolute);

    match artifacts::create(
        pool,
        CreateArtifactInput {
            workspace_id: workspace_id.to_string(),
            task_id: task_id.map(str::to_string),
            run_id: run_id.to_string(),
            file_path: relative,
            absolute_path: absolute.to_string_lossy().to_string(),
            file_type,
            file_size_bytes: size,
            content_sha256: hash,
            previous_sha256: previous_sha256.map(str::to_string),
            action_type: action.to_string(),
            created_by_agent: agent.to_string(),
            creation_reason: reason.to_string(),
            diff_patch: diff_patch.map(str::to_string),
        },
    )
    .await
    {
        Ok(artifact) => events.publish(OpenDeskWsEvent::ArtifactCreated {
            artifact: artifact_dto(&artifact),
        }),
        Err(error) => tracing::warn!(error = %error, "could not record artifact"),
    }
}

fn file_fingerprint(path: &Path) -> (i64, Option<String>) {
    let Ok(metadata) = std::fs::metadata(path) else {
        return (0, None);
    };
    if !metadata.is_file() {
        return (0, None);
    }
    let hash = std::fs::read(path).ok().map(|bytes| {
        let digest = Sha256::digest(&bytes);
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    });
    (metadata.len() as i64, hash)
}

fn artifact_dto(artifact: &artifacts::ArtifactProvenance) -> ArtifactProvenanceDto {
    ArtifactProvenanceDto {
        id: artifact.id.clone(),
        workspace_id: artifact.workspace_id.clone(),
        task_id: artifact.task_id.clone(),
        run_id: artifact.run_id.clone(),
        file_path: artifact.file_path.clone(),
        absolute_path: artifact.absolute_path.clone(),
        file_type: artifact.file_type.clone(),
        action_type: artifact.action_type.clone(),
        created_by_agent: artifact.created_by_agent.clone(),
        creation_reason: artifact.creation_reason.clone(),
        diff_patch: artifact.diff_patch.clone(),
        created_at: artifact.created_at.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job() -> queue::QueueJob {
        queue::QueueJob {
            id: "job".into(),
            workspace_id: "ws".into(),
            task_id: None,
            schedule_id: None,
            session_id: None,
            concurrency_group: None,
            prompt_payload: String::new(),
            resolved_mode: "BUILD_DIRECT".into(),
            resolved_agent: "build".into(),
            resolved_model: "provider/model".into(),
            resolved_effort: "low".into(),
            resolved_harness: "opencode".into(),
            assigned_node_id: None,
            job_kind: "prompt".into(),
            scheduled_for: None,
            output_dir: None,
            priority: 1,
            max_retries: 0,
            retry_count: 0,
            status: "QUEUED".into(),
            task_title: Some("Tarefa".into()),
            enqueued_at: String::new(),
            started_at: None,
            finished_at: None,
            lease_expires_at: None,
            last_heartbeat_at: None,
            error_message: None,
            task_origin: Some("MANUAL".into()),
        }
    }

    #[test]
    fn plan_uses_the_job_mode_and_slugged_title() {
        let temp = tempfile::TempDir::new().expect("temp");
        let paths = AppPaths {
            data_dir: temp.path().join("data"),
            config_dir: temp.path().join("config"),
            cache_dir: temp.path().join("cache"),
            log_dir: temp.path().join("logs"),
            db_path: temp.path().join("data/opendesk.db"),
            backups_dir: temp.path().join("backups"),
            artifacts_dir: temp.path().join("artifacts"),
            run_logs_dir: temp.path().join("runs"),
            daemon_lock_path: temp.path().join("daemon.json"),
            api_lock_path: temp.path().join("api.json"),
            allow_temp_workspaces: true,
        };
        let plan = plan_run(&job(), None, &paths, temp.path());
        assert_eq!(plan.phase, "BUILD_DIRECT");
        assert!(plan.log_path.ends_with(format!("{}.jsonl", plan.run_id)));
        assert!(plan.output_dir.ends_with("tarefa"));
    }

    #[tokio::test]
    async fn records_diffs_and_writes_once() {
        use crate::opencode_client::rest_client::FileDiff;
        use crate::opencode_client::session_runner::{FileWrite, RunStatus};

        let context = crate::test_support::context().await;
        crate::db::runs::create(
            &context.db,
            crate::db::runs::CreateRunInput {
                id: "run-artifacts".into(),
                queue_job_id: None,
                workspace_id: context.workspace_id.clone(),
                task_id: None,
                schedule_id: None,
                parent_run_id: None,
                phase: Some("build".into()),
                agent_name: "build".into(),
                execution_mode: "BUILD_DIRECT".into(),
                provider_id: Some("opencode-go".into()),
                model_id: "glm-5.3".into(),
                effort_level: "low".into(),
                pricing_snapshot_json: None,
                log_path: None,
            },
        )
        .await
        .expect("run");
        std::fs::write(context.workspace_root.join("a.md"), "a").expect("a");
        std::fs::write(context.workspace_root.join("b.txt"), "b").expect("b");
        let mut job = job();
        job.workspace_id = context.workspace_id.clone();
        let outcome = RunOutcome {
            status: RunStatus::Success,
            session_id: "session".into(),
            usage: crate::db::runs::RunUsage::default(),
            summary: None,
            diffs: vec![FileDiff {
                file: "a.md".into(),
                patch: "+a".into(),
                additions: 1,
                deletions: 0,
                status: "added".into(),
            }],
            writes: vec![
                FileWrite {
                    path: "b.txt".into(),
                    action: "MODIFIED".into(),
                    diff_patch: None,
                },
                FileWrite {
                    path: "a.md".into(),
                    action: "MODIFIED".into(),
                    diff_patch: None,
                },
            ],
            error: None,
            log_tail: None,
            permissions: Vec::new(),
        };
        record_artifacts(
            &context.db,
            &context.events,
            &job,
            "run-artifacts",
            "teste",
            &context.workspace_root,
            &context.workspace_root.join("no-output"),
            &outcome,
        )
        .await;
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM artifacts_provenance WHERE run_id = 'run-artifacts'",
        )
        .fetch_one(&context.db)
        .await
        .expect("count");
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn scans_shell_created_files_in_the_output_directory() {
        use crate::opencode_client::session_runner::{RunOutcome, RunStatus};

        let context = crate::test_support::context().await;
        let output = context.workspace_root.join("output").join("tarefa");
        std::fs::create_dir_all(&output).expect("output");
        std::fs::write(output.join("poesia.txt"), "v1").expect("file");
        let mut job = job();
        job.workspace_id = context.workspace_id.clone();

        let empty = || RunOutcome {
            status: RunStatus::Success,
            session_id: "session".into(),
            usage: crate::db::runs::RunUsage::default(),
            summary: None,
            diffs: Vec::new(),
            writes: Vec::new(),
            error: None,
            log_tail: None,
            permissions: Vec::new(),
        };
        let create_run = |id: &'static str| {
            let db = context.db.clone();
            let workspace_id = context.workspace_id.clone();
            async move {
                crate::db::runs::create(
                    &db,
                    crate::db::runs::CreateRunInput {
                        id: id.into(),
                        queue_job_id: None,
                        workspace_id,
                        task_id: None,
                        schedule_id: None,
                        parent_run_id: None,
                        phase: Some("build".into()),
                        agent_name: "build".into(),
                        execution_mode: "BUILD_DIRECT".into(),
                        provider_id: Some("opencode-go".into()),
                        model_id: "glm-5.3".into(),
                        effort_level: "low".into(),
                        pricing_snapshot_json: None,
                        log_path: None,
                    },
                )
                .await
                .expect("run");
            }
        };

        create_run("run-scan-1").await;
        record_artifacts(
            &context.db,
            &context.events,
            &job,
            "run-scan-1",
            "teste",
            &context.workspace_root,
            &output,
            &empty(),
        )
        .await;
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM artifacts_provenance WHERE run_id = 'run-scan-1'",
        )
        .fetch_one(&context.db)
        .await
        .expect("count");
        assert_eq!(count, 1);
        let artifact = crate::db::artifacts::latest_for_path(
            &context.db,
            &context.workspace_id,
            "output/tarefa/poesia.txt",
        )
        .await
        .expect("artifact")
        .expect("artifact row");
        assert_eq!(artifact.action_type, "CREATED");

        // Re-running the same run must not duplicate rows.
        record_artifacts(
            &context.db,
            &context.events,
            &job,
            "run-scan-1",
            "teste",
            &context.workspace_root,
            &output,
            &empty(),
        )
        .await;
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM artifacts_provenance WHERE run_id = 'run-scan-1'",
        )
        .fetch_one(&context.db)
        .await
        .expect("count");
        assert_eq!(count, 1);

        create_run("run-scan-2").await;
        std::fs::write(output.join("poesia.txt"), "v2").expect("file");
        record_artifacts(
            &context.db,
            &context.events,
            &job,
            "run-scan-2",
            "teste",
            &context.workspace_root,
            &output,
            &empty(),
        )
        .await;
        let artifact = crate::db::artifacts::latest_for_path(
            &context.db,
            &context.workspace_id,
            "output/tarefa/poesia.txt",
        )
        .await
        .expect("artifact")
        .expect("artifact row");
        assert_eq!(artifact.run_id, "run-scan-2");
        assert_eq!(artifact.action_type, "MODIFIED");
        assert!(artifact.previous_sha256.is_some());
    }

    #[tokio::test]
    async fn chat_artifacts_are_linked_to_a_chat_run() {
        use crate::opencode_client::rest_client::FileDiff;

        let context = crate::test_support::context().await;
        std::fs::write(context.workspace_root.join("chat.md"), "oi").expect("file");
        let run = crate::db::runs::create_chat(
            &context.db,
            crate::db::runs::CreateChatRunInput {
                id: "run-chat-1".into(),
                workspace_id: context.workspace_id.clone(),
                session_id: Some("ses-chat".into()),
                agent_name: "build".into(),
                provider_id: Some("opencode-go".into()),
                model_id: "deepseek".into(),
                channel: "telegram".into(),
                origin_ref: "thread-1".into(),
                started_at: chrono::Utc::now().to_rfc3339(),
            },
        )
        .await
        .expect("chat run");
        record_chat_artifacts(
            &context.db,
            &context.events,
            &context.workspace_id,
            &run.id,
            "build",
            "Chat · telegram",
            &context.workspace_root,
            &[FileDiff {
                file: "chat.md".into(),
                patch: "+oi".into(),
                additions: 1,
                deletions: 0,
                status: "added".into(),
            }],
            &[],
        )
        .await;
        let artifact =
            crate::db::artifacts::latest_for_path(&context.db, &context.workspace_id, "chat.md")
                .await
                .expect("artifact")
                .expect("artifact row");
        assert_eq!(artifact.source, "chat");
        assert_eq!(artifact.channel.as_deref(), Some("telegram"));
        assert_eq!(artifact.origin_ref.as_deref(), Some("thread-1"));
    }
}
