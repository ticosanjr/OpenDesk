use super::super::{AgendaItem, rfc3339, status_for};
use crate::{
    db::{queue::QueueJob, runs::TaskRun},
    recipes::store::RecipeRun,
};
use chrono::{DateTime, Utc};

pub(super) fn execution_color(status: &str) -> Option<String> {
    match status.to_uppercase().as_str() {
        "SUCCESS" | "DONE" => Some("#10b981".into()),
        "FAILED" | "BLOCKED" => Some("#ef4444".into()),
        "RUNNING" | "ACTIVE" => Some("#3b82f6".into()),
        "QUEUED" | "PAUSED" | "MISSED" | "PENDING" | "WAITING_HITL" => Some("#f59e0b".into()),
        "CANCELLED" | "IGNORED" => Some("#71717a".into()),
        _ => None,
    }
}

pub(super) fn missed_status(status: &str) -> &'static str {
    match status {
        "PENDING" => "MISSED",
        "QUEUED" => "QUEUED",
        "FAILED" => "FAILED",
        "CANCELLED" => "CANCELLED",
        _ => "MISSED",
    }
}

pub(super) fn run_item(
    run: &TaskRun,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    now: DateTime<Utc>,
) -> AgendaItem {
    AgendaItem {
        id: run.id.clone(),
        kind: "run".into(),
        title: run
            .task_title
            .clone()
            .unwrap_or_else(|| format!("Execução {}", &run.id[..8.min(run.id.len())])),
        subtitle: Some(format!("@{} · {}", run.agent_name, run.model_id)),
        start: rfc3339(start),
        end: Some(rfc3339(end)),
        all_day: false,
        status: status_for(start, end, now),
        source: "EXECUTION".into(),
        color: execution_color(&run.status),
        href: Some(format!("/runs/{}", run.id)),
        execution_status: Some(run.status.clone()),
        run_id: Some(run.id.clone()),
        missed_id: None,
        executed: true,
        editable: false,
    }
}

pub(super) fn recipe_run_item(
    run: &RecipeRun,
    title: String,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    now: DateTime<Utc>,
) -> AgendaItem {
    AgendaItem {
        id: run.id.clone(),
        kind: "recipe_run".into(),
        title,
        subtitle: Some(format!("receita · {}", run.status)),
        start: rfc3339(start),
        end: Some(rfc3339(end)),
        all_day: false,
        status: status_for(start, end, now),
        source: "EXECUTION".into(),
        color: execution_color(&run.status),
        href: Some(format!("/recipes?recipe={}&run={}", run.recipe_id, run.id)),
        execution_status: Some(run.status.clone()),
        run_id: Some(run.id.clone()),
        missed_id: None,
        executed: true,
        editable: false,
    }
}

pub(super) fn queued_item(
    job: &QueueJob,
    scheduled: DateTime<Utc>,
    end: DateTime<Utc>,
    now: DateTime<Utc>,
) -> AgendaItem {
    AgendaItem {
        id: job.id.clone(),
        kind: "queued".into(),
        title: job
            .task_title
            .clone()
            .unwrap_or_else(|| "Execução agendada".into()),
        subtitle: Some(format!("@{} · fila {}", job.resolved_agent, job.status)),
        start: rfc3339(scheduled),
        end: Some(rfc3339(end)),
        all_day: false,
        status: status_for(scheduled, end, now),
        source: "EXECUTION".into(),
        color: execution_color(&job.status),
        href: Some(format!("/kanban?view=queue&job={}", job.id)),
        execution_status: Some(job.status.clone()),
        run_id: None,
        missed_id: None,
        executed: false,
        editable: false,
    }
}
