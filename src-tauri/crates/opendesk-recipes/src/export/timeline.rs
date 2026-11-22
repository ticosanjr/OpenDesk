use crate::error::AppResult;

use opendesk_engine::run_timeline::{
    RunAttempt, RunTimeline, TimelineArtifact, timeline_from_events,
};
use opendesk_storage::repos::{
    artifacts, queue,
    runs::{self, TaskRun},
    tasks, workspaces,
};

use sqlx::SqlitePool;
use std::path::Path;

use super::cache::cached_events;
use super::inputs::load_run_inputs;

pub async fn load_timeline(pool: &SqlitePool, id: &str) -> AppResult<RunTimeline> {
    let run = runs::get(pool, id).await?;
    let mut timeline = build_timeline(pool, run.clone()).await?;
    timeline.root_run_id = Some(run.id.clone());
    timeline.attempts = vec![attempt_from_run(&run)];
    Ok(timeline)
}

/// One logical execution can contain several attempts (follow-ups, retries).
/// Merge all attempts of the task into a single chronological timeline while
/// keeping per-attempt summaries for the header.
pub async fn load_thread_timeline(pool: &SqlitePool, id: &str) -> AppResult<RunTimeline> {
    let requested = runs::get(pool, id).await?;
    let mut attempts = match requested.task_id.as_deref() {
        Some(task_id) => runs::list(pool, &requested.workspace_id, Some(task_id))
            .await
            .unwrap_or_else(|_| vec![requested.clone()]),
        None => vec![requested.clone()],
    };
    attempts.sort_by(|left, right| {
        left.started_at
            .cmp(&right.started_at)
            .then(left.attempt_number.cmp(&right.attempt_number))
    });
    if attempts.is_empty() {
        attempts.push(requested);
    }

    let mut merged = RunTimeline {
        root_run_id: attempts.first().map(|run| run.id.clone()),
        attempts: attempts.iter().map(attempt_from_run).collect(),
        ..Default::default()
    };
    let mut first_started: Option<String> = None;
    let mut last_completed: Option<String> = None;
    let mut events_count = 0;
    let mut duration_ms = 0;
    let mut log_missing = false;

    for run in &attempts {
        let mut part = build_timeline(pool, run.clone()).await?;
        for event in &mut part.events {
            event.run_id = Some(run.id.clone());
            event.attempt_number = Some(run.attempt_number);
        }
        for step in &mut part.steps {
            step.attempt_number = Some(run.attempt_number);
        }
        events_count += part.events_count;
        duration_ms += part.duration_ms;
        log_missing |= part.log_missing;
        if first_started.is_none() {
            first_started = part.started_at.clone();
        }
        if part.completed_at.is_some() {
            last_completed = part.completed_at.clone();
        }
        merged.events.append(&mut part.events);
        merged.steps.append(&mut part.steps);
        merged.artifacts.append(&mut part.artifacts);
        if !part.result.text.trim().is_empty() {
            merged.result = part.result;
        }
        if !part.thinking.text.trim().is_empty() {
            merged.thinking = part.thinking;
        }
        if !part.executed.tools.is_empty() {
            merged.executed.tools.extend(part.executed.tools);
        }
        merged.workspace_root = part.workspace_root.or(merged.workspace_root);
        merged.output_directory = part.output_directory.or(merged.output_directory);
        merged.output_directory_relative = part
            .output_directory_relative
            .or(merged.output_directory_relative);
    }

    merged.events.sort_by(|left, right| {
        left.started_at
            .as_deref()
            .unwrap_or_default()
            .cmp(right.started_at.as_deref().unwrap_or_default())
    });
    merged.steps.sort_by(|left, right| {
        left.started_at
            .as_deref()
            .unwrap_or_default()
            .cmp(right.started_at.as_deref().unwrap_or_default())
    });
    merged.executed.tools.sort_by(|left, right| {
        left.started_at
            .as_deref()
            .unwrap_or_default()
            .cmp(right.started_at.as_deref().unwrap_or_default())
    });
    merged.run = attempts.last().cloned();
    merged.status = merged
        .run
        .as_ref()
        .map(|run| run.status.clone())
        .unwrap_or_default();
    merged.started_at = first_started;
    merged.completed_at = last_completed;
    merged.duration_ms = duration_ms;
    merged.events_count = events_count;
    merged.log_missing = log_missing;
    Ok(merged)
}

fn attempt_from_run(run: &TaskRun) -> RunAttempt {
    RunAttempt {
        id: run.id.clone(),
        attempt_number: run.attempt_number,
        status: run.status.clone(),
        phase: run.phase.clone(),
        started_at: run.started_at.clone(),
        completed_at: run.completed_at.clone(),
        duration_ms: run.duration_ms,
        tokens_input: run.tokens_input,
        tokens_output: run.tokens_output,
        cost_usd: run.estimated_cost_usd,
        error_message: run.error_message.clone(),
    }
}

async fn build_timeline(pool: &SqlitePool, run: TaskRun) -> AppResult<RunTimeline> {
    let path = run.log_path.clone();
    let events = path
        .as_deref()
        .and_then(|path| cached_events(Path::new(path)));
    let log_missing = events.is_none();
    let mut timeline =
        timeline_from_events(Some(run.clone()), events.unwrap_or_default(), log_missing);

    let workspace = workspaces::get(pool, &run.workspace_id).await.ok();
    timeline.inputs = load_run_inputs(pool, &run, workspace.as_ref()).await;
    if let Some(workspace) = &workspace {
        timeline.workspace_root = Some(workspace.root_path.clone());
    }
    let mut output_directory = match run.task_id.as_deref() {
        Some(task_id) => tasks::get(pool, task_id)
            .await
            .ok()
            .and_then(|task| task.output_dir_path),
        None => None,
    };
    if output_directory.is_none()
        && let Some(job_id) = run.queue_job_id.as_deref()
    {
        output_directory = queue::get(pool, job_id)
            .await
            .ok()
            .and_then(|job| job.output_dir);
    }
    if let Some(output) = &output_directory {
        timeline.output_directory = Some(output.clone());
        if let Some(workspace) = &workspace {
            timeline.output_directory_relative = output
                .strip_prefix(&workspace.root_path)
                .map(str::to_string);
        }
    }
    timeline.artifacts = artifacts::list_by_run(pool, &run.id, 200)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|artifact| TimelineArtifact {
            id: artifact.id,
            file_path: artifact.file_path,
            absolute_path: artifact.absolute_path,
            action_type: artifact.action_type,
            created_at: artifact.created_at,
        })
        .collect();
    Ok(timeline)
}
