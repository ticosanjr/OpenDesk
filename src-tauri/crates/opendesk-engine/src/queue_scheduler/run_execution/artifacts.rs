use crate::opencode_client::session_runner::RunOutcome;
use opendesk_storage::events::{ArtifactProvenanceDto, EventBus, OpenDeskWsEvent};
use opendesk_storage::repos::{
    artifacts::{self, CreateArtifactInput},
    queue,
};

use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

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
