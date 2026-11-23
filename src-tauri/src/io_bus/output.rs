use super::{IoContext, outbound};
use crate::{db, error::AppResult};

#[derive(Debug, Clone, Default)]
pub struct RunOutput {
    pub workspace_id: String,
    pub workspace_root: String,
    pub output_directory: Option<String>,
    pub task_id: Option<String>,
    pub run_id: String,
    pub title: String,
    pub summary: String,
    pub artifacts: Vec<RunArtifact>,
}

#[derive(Debug, Clone)]
pub struct RunArtifact {
    pub file_path: String,
    pub absolute_path: String,
    pub action_type: String,
}

pub async fn load_run_output(
    ctx: &IoContext,
    workspace_id: &str,
    task_id: Option<&str>,
    _schedule_id: Option<&str>,
    run_id: &str,
) -> AppResult<RunOutput> {
    let run = db::runs::get(&ctx.pool, run_id).await?;
    let workspace = db::workspaces::get(&ctx.pool, workspace_id).await?;
    let output_directory = if let Some(task_id) = task_id {
        db::tasks::get(&ctx.pool, task_id)
            .await
            .ok()
            .and_then(|task| task.output_dir_path)
    } else {
        None
    };
    let artifacts = db::artifacts::list_by_run(&ctx.pool, run_id, 500)
        .await?
        .into_iter()
        .map(|artifact| RunArtifact {
            file_path: artifact.file_path,
            absolute_path: artifact.absolute_path,
            action_type: artifact.action_type,
        })
        .collect();
    Ok(RunOutput {
        workspace_id: workspace_id.to_string(),
        workspace_root: workspace.root_path,
        output_directory,
        task_id: task_id.map(str::to_string),
        run_id: run_id.to_string(),
        title: run
            .task_title
            .clone()
            .unwrap_or_else(|| "OpenDesk run".into()),
        summary: run.summary_response.unwrap_or_default(),
        artifacts,
    })
}

/// Execute all sinks bound to a task/schedule after a successful run. Sink
/// failures are isolated and never change the run status.
pub async fn run_sinks(
    ctx: &IoContext,
    workspace_id: &str,
    task_id: Option<&str>,
    schedule_id: Option<&str>,
    run_id: &str,
) -> AppResult<()> {
    let sinks = db::io_bus::sinks_for_run(&ctx.pool, task_id, schedule_id).await?;
    if sinks.is_empty() {
        return Ok(());
    }
    let output = load_run_output(ctx, workspace_id, task_id, schedule_id, run_id).await?;
    for sink in sinks {
        let result = outbound::execute(ctx, &sink, &output).await;
        match &result {
            Ok(_) => {
                let _ = db::io_bus::mark_outbound(&ctx.pool, &sink.id, "SUCCESS", None).await;
            }
            Err(error) => {
                tracing::warn!(sink = %sink.id, run = %run_id, error = %error, "outbound sink failed");
                let _ = db::io_bus::mark_outbound(
                    &ctx.pool,
                    &sink.id,
                    "FAILED",
                    Some(&format!("{error:#}")),
                )
                .await;
            }
        }
    }
    Ok(())
}
