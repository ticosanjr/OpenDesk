use crate::{
    db::{queue, runs, schedules, tasks},
    events::{EventBus, OpenDeskWsEvent},
    opencode_client::session_runner::{RunOutcome, RunStatus},
};
use sqlx::SqlitePool;
use uuid::Uuid;

/// Queue status for a completed run.
pub fn queue_status(outcome: &RunOutcome) -> &'static str {
    match outcome.status {
        RunStatus::Success => "COMPLETED",
        RunStatus::Failed => "FAILED",
        RunStatus::Cancelled => "CANCELLED",
    }
}

/// Routine status for a completed run.
pub fn schedule_status(outcome: &RunOutcome) -> &'static str {
    match outcome.status {
        RunStatus::Success => "SUCCESS",
        RunStatus::Failed => "FAILED",
        RunStatus::Cancelled => "CANCELLED",
    }
}

pub fn classify_run_error(message: &str) -> (&'static str, &'static str) {
    let lower = message.to_lowercase();
    if lower.contains("daemon is offline") || lower.contains("connection refused") {
        (
            "engine_offline",
            "O motor OpenCode não está acessível. Reinicie o motor em Configurações → OpenCode Engine.",
        )
    } else if lower.contains("401") || lower.contains("unauthorized") || lower.contains("password")
    {
        (
            "auth_failed",
            "O OpenCode rejeitou a autenticação. Reinicie o motor para renovar a senha do daemon.",
        )
    } else if lower.contains("model") && (lower.contains("not found") || lower.contains("unknown"))
    {
        (
            "model_not_found",
            "O modelo configurado não existe ou não está disponível para este workspace. Edite a tarefa e escolha outro modelo.",
        )
    } else if lower.contains("agent") && lower.contains("not found") {
        (
            "agent_not_found",
            "O agente configurado não existe no workspace. Edite a tarefa e escolha outro agente.",
        )
    } else if lower.contains("429")
        || lower.contains("too many requests")
        || lower.contains("503")
        || lower.contains("service unavailable")
    {
        (
            "rate_limited",
            "O provedor limitou temporariamente o modelo. O OpenDesk tenta um fallback configurado; se não houver, a tarefa fica bloqueada.",
        )
    } else if lower.contains("timed out") || lower.contains("timeout") {
        (
            "timeout",
            "A execução excedeu o tempo limite. Tente novamente ou reduza o escopo da tarefa.",
        )
    } else if lower.contains("cancel") {
        (
            "cancelled",
            "A execução foi cancelada. Use Tentar de novo para enfileirar outra execução.",
        )
    } else if lower.contains("budget") {
        (
            "budget_blocked",
            "O budget guard bloqueou a execução. Ajuste o limite em Roteador & Custos ou aguarde o próximo período.",
        )
    } else if lower.contains("no such file") || lower.contains("enoent") {
        (
            "binary_missing",
            "O binário do OpenCode não foi encontrado. Configure o caminho em Configurações → OpenCode Engine.",
        )
    } else {
        (
            "run_failed",
            "Abra o log completo e o diagnóstico da tarefa. Se o erro citar modelo/agente, edite a tarefa e tente novamente.",
        )
    }
}

pub async fn record_permissions(pool: &SqlitePool, run_id: &str, outcome: &RunOutcome) {
    for permission in &outcome.permissions {
        let _ = sqlx::query(
            "INSERT INTO permission_audit (
                id, run_id, session_id, request_id, permission_action, resource, decision
             ) VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::now_v7().to_string())
        .bind(run_id)
        .bind(&outcome.session_id)
        .bind(&permission.request_id)
        .bind(&permission.action)
        .bind(permission.resources.join(","))
        .bind(&permission.decision)
        .execute(pool)
        .await;
    }
}

pub async fn finish_run(pool: &SqlitePool, run_id: &str, outcome: &RunOutcome, duration_ms: i64) {
    let _ = runs::finish(
        pool,
        run_id,
        outcome.status.as_str(),
        &outcome.usage,
        duration_ms,
        outcome.summary.as_deref(),
        outcome.log_tail.as_deref(),
    )
    .await;
}

pub async fn mark_schedule(
    pool: &SqlitePool,
    events: &EventBus,
    job: &queue::QueueJob,
    status: &str,
) {
    let Some(schedule_id) = job.schedule_id.as_deref() else {
        return;
    };
    let _ = schedules::mark_run(pool, schedule_id, status, &chrono::Utc::now().to_rfc3339()).await;
    events.publish(OpenDeskWsEvent::ScheduleUpdated {
        schedule_id: schedule_id.to_string(),
        workspace_id: job.workspace_id.clone(),
    });
}

pub async fn settle_queue(
    pool: &SqlitePool,
    events: &EventBus,
    job: &queue::QueueJob,
    outcome: &RunOutcome,
) {
    let _ = queue::finish(
        pool,
        &job.id,
        queue_status(outcome),
        outcome.error.as_deref(),
    )
    .await;
    mark_schedule(pool, events, job, schedule_status(outcome)).await;
}

pub async fn settle_task(
    pool: &SqlitePool,
    events: &EventBus,
    task: &tasks::Task,
    phase: &str,
    outcome: &RunOutcome,
    run_id: &str,
    fallback_enqueued: bool,
) {
    if fallback_enqueued {
        return;
    }
    let _ = tasks::set_last_run(pool, &task.id, Some(run_id), Some(outcome.status.as_str())).await;

    let (next_status, phase_status) = match (phase, outcome.status) {
        ("plan", RunStatus::Success) => {
            tasks::set_plan(
                pool,
                &task.id,
                outcome
                    .summary
                    .as_deref()
                    .unwrap_or("Plano concluído sem texto retornado pelo modelo."),
            )
            .await
            .ok();
            if task.execution_mode == "PLAN_ONLY" {
                ("DONE", "plan_complete")
            } else {
                ("IN_REVIEW", "awaiting_approval")
            }
        }
        (_, RunStatus::Success) => (
            "DONE",
            if phase == "audit" {
                "audit_approved"
            } else {
                "build_complete"
            },
        ),
        (_, RunStatus::Cancelled) => ("CANCELLED", "cancelled"),
        (_, RunStatus::Failed) => ("BLOCKED", "failed"),
    };

    if next_status == "CANCELLED" {
        let _ = tasks::set_cancelled(pool, &task.id).await;
    } else {
        let _ =
            tasks::set_phase_status(pool, &task.id, Some(phase_status), Some(next_status)).await;
    }
    if next_status == "BLOCKED" {
        let message = outcome
            .error
            .as_deref()
            .or(outcome.summary.as_deref())
            .unwrap_or("run failed without a message");
        let _ = tasks::set_blocked_reason(pool, &task.id, Some(message)).await;
    } else {
        let _ = tasks::set_blocked_reason(pool, &task.id, None).await;
    }

    events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: next_status.into(),
        phase_status: Some(phase_status.into()),
    });
}

pub fn publish_run_finished(
    events: &EventBus,
    job: &queue::QueueJob,
    run_id: &str,
    outcome: &RunOutcome,
) {
    events.publish(OpenDeskWsEvent::RunFinished {
        run_id: run_id.to_string(),
        task_id: job.task_id.clone(),
        status: outcome.status.as_str().to_string(),
        estimated_cost_usd: outcome.usage.estimated_cost_usd,
        tokens_input: outcome.usage.tokens_input,
        tokens_output: outcome.usage.tokens_output,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tasks::{self as task_store, CreateTaskInput};

    fn outcome(status: RunStatus, summary: Option<&str>) -> RunOutcome {
        RunOutcome {
            status,
            session_id: "session".into(),
            usage: runs::RunUsage::default(),
            summary: summary.map(str::to_string),
            diffs: Vec::new(),
            writes: Vec::new(),
            error: None,
            log_tail: None,
            permissions: Vec::new(),
        }
    }

    #[test]
    fn maps_run_status_to_queue_and_schedule_state() {
        assert_eq!(
            queue_status(&outcome(RunStatus::Success, None)),
            "COMPLETED"
        );
        assert_eq!(queue_status(&outcome(RunStatus::Failed, None)), "FAILED");
        assert_eq!(
            schedule_status(&outcome(RunStatus::Cancelled, None)),
            "CANCELLED"
        );
    }

    #[test]
    fn classifies_actionable_errors() {
        assert_eq!(
            classify_run_error("OpenCode daemon is offline").0,
            "engine_offline"
        );
        assert_eq!(classify_run_error("429 rate limit").0, "rate_limited");
        assert_eq!(
            classify_run_error("model not found: x").0,
            "model_not_found"
        );
        assert_eq!(
            classify_run_error("agent not found: x").0,
            "agent_not_found"
        );
        assert_eq!(classify_run_error("timed out").0, "timeout");
        assert_eq!(classify_run_error("cancelled by user").0, "cancelled");
        assert_eq!(classify_run_error("budget stop").0, "budget_blocked");
        assert_eq!(classify_run_error("ENOENT").0, "binary_missing");
        assert_eq!(classify_run_error("unknown").0, "run_failed");
    }

    #[tokio::test]
    async fn plan_success_waits_for_approval_and_build_success_completes() {
        let context = crate::test_support::context().await;
        let task = task_store::create(
            &context.state.db,
            &context.workspace_id,
            CreateTaskInput {
                title: "Plan".into(),
                description: "teste".into(),
                execution_mode: Some("AUTO_DSH".into()),
                priority: None,
                assigned_agent: None,
                preferred_model: None,
                effort_level: None,
                notify_email: None,
                export_to_gdocs: false,
                tags: Vec::new(),
            },
        )
        .await
        .expect("task");
        let success = outcome(RunStatus::Success, Some("plano"));
        settle_task(
            &context.state.db,
            &context.state.events,
            &task,
            "plan",
            &success,
            "run-plan",
            false,
        )
        .await;
        let planned = task_store::get(&context.state.db, &task.id)
            .await
            .expect("planned");
        assert_eq!(planned.status, "IN_REVIEW");
        assert_eq!(planned.phase_status.as_deref(), Some("awaiting_approval"));

        settle_task(
            &context.state.db,
            &context.state.events,
            &planned,
            "build",
            &success,
            "run-build",
            false,
        )
        .await;
        let built = task_store::get(&context.state.db, &task.id)
            .await
            .expect("built");
        assert_eq!(built.status, "DONE");
        assert_eq!(built.phase_status.as_deref(), Some("build_complete"));
    }

    #[tokio::test]
    async fn failed_run_blocks_the_task_with_the_error() {
        let context = crate::test_support::context().await;
        let task = task_store::create(
            &context.state.db,
            &context.workspace_id,
            CreateTaskInput {
                title: "Falha".into(),
                description: "teste".into(),
                execution_mode: Some("BUILD_DIRECT".into()),
                priority: None,
                assigned_agent: None,
                preferred_model: None,
                effort_level: None,
                notify_email: None,
                export_to_gdocs: false,
                tags: Vec::new(),
            },
        )
        .await
        .expect("task");
        let mut failed = outcome(RunStatus::Failed, None);
        failed.error = Some("rate limit".into());
        settle_task(
            &context.state.db,
            &context.state.events,
            &task,
            "build",
            &failed,
            "run-failed",
            false,
        )
        .await;
        let blocked = task_store::get(&context.state.db, &task.id)
            .await
            .expect("blocked");
        assert_eq!(blocked.status, "BLOCKED");
        assert_eq!(blocked.blocked_reason.as_deref(), Some("rate limit"));
    }
}
