use crate::{
    app_state::AppState,
    db::{queue, runs, tasks},
    error::AppResult,
};
use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

pub async fn task_diagnostics(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let task = tasks::get(&state.db, &id).await?;
    let jobs = queue::list(&state.db, Some(&task.workspace_id)).await?;
    let jobs = jobs
        .into_iter()
        .filter(|job| job.task_id.as_deref() == Some(task.id.as_str()))
        .collect::<Vec<_>>();
    let runs = runs::list(&state.db, &task.workspace_id, Some(&task.id)).await?;
    let engine = state.daemon.status().await;

    let failed_run = runs
        .iter()
        .find(|run| run.status == "FAILED" || run.status == "CANCELLED");
    let failed_job = jobs
        .iter()
        .find(|job| matches!(job.status.as_str(), "FAILED" | "CANCELLED"));

    let error = failed_run
        .and_then(|run| {
            let message = run.error_message.clone().or_else(|| run.summary_response.clone())?;
            Some(serde_json::json!({
                "code": run.error_code.clone().unwrap_or_else(|| "run_failed".into()),
                "message": message,
                "hint": run.error_hint.clone().unwrap_or_else(|| "Abra o log completo e tente novamente.".into()),
                "source": "run",
                "run_id": run.id,
            }))
        })
        .or_else(|| {
            failed_job.and_then(|job| {
                job.error_message.as_ref().map(|message| {
                    serde_json::json!({
                        "code": "queue_failed",
                        "message": message,
                        "hint": "Use Tentar de novo no controlador da fila ou reexecute a tarefa.",
                        "source": "queue",
                        "job_id": job.id,
                    })
                })
            })
        });

    let latest_log_tail = runs
        .iter()
        .find_map(|run| run.log_path.as_deref())
        .and_then(|path| read_tail(std::path::Path::new(path), 64 * 1024));

    Ok(Json(serde_json::json!({
        "task": task,
        "jobs": jobs,
        "runs": runs,
        "engine": engine,
        "error": error,
        "latest_log_tail": latest_log_tail,
        "generated_at": chrono::Utc::now().to_rfc3339(),
    })))
}

fn read_tail(path: &std::path::Path, max_bytes: u64) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    let start = length.saturating_sub(max_bytes);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).ok()?;
    let text = String::from_utf8_lossy(&buffer).to_string();
    if start > 0
        && let Some(index) = text.find('\n')
    {
        return Some(text[index + 1..].to_string());
    }
    Some(text)
}
