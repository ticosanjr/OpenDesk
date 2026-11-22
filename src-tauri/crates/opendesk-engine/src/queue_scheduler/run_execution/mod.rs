use opendesk_storage::paths::AppPaths;
use opendesk_storage::repos::{queue, tasks};

use std::path::{Path, PathBuf};
use uuid::Uuid;

mod artifacts;

pub use artifacts::{record_artifact, record_artifacts, record_chat_artifacts};

#[cfg(test)]
mod tests;

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
