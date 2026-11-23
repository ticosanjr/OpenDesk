use crate::{
    app_state::AppState,
    db::tasks::{self, CreateTaskInput},
    error::AppResult,
};
use std::sync::Arc;

use super::goals::active_goal_summary;
use super::metrics::check_budget;
use super::profiles::{due_heartbeats, mark_heartbeat};

fn heartbeat_prompt(agent_id: &str, goals: &str) -> (String, String) {
    (
        format!("Heartbeat · @{agent_id}"),
        format!(
            "Você é @{agent_id} no seu ciclo periódico de governança.\n\n## Metas ativas\n\n{goals}\n\n## Tarefa do heartbeat\n\n1. Revise o estado atual e o que já foi entregue.\n2. Identifique o próximo passo de maior impacto para cada meta.\n3. Proponha ações concretas, riscos e o que precisa de aprovação humana.\n4. Não execute ações destrutivas nem altere configurações sem propor HITL.\n5. Responda de forma curta e acionável; artefatos vão para a pasta de saída do run."
        ),
    )
}

/// Creates and dispatches one heartbeat task for an agent.
pub async fn run_heartbeat(
    state: &Arc<AppState>,
    workspace_id: &str,
    agent_id: &str,
) -> AppResult<tasks::Task> {
    check_budget(&state.db, workspace_id, agent_id).await?;
    let goals = active_goal_summary(&state.db, workspace_id, agent_id).await?;
    let (title, description) = heartbeat_prompt(agent_id, &goals);
    let task = tasks::create(
        &state.db,
        workspace_id,
        CreateTaskInput {
            title,
            description,
            execution_mode: Some("BUILD_DIRECT".into()),
            priority: Some(2),
            assigned_agent: Some(agent_id.to_string()),
            preferred_model: None,
            preferred_harness: None,
            effort_level: None,
            care_level: Some("standard".into()),
            notify_email: None,
            export_to_gdocs: false,
            tags: vec!["governance:heartbeat".into()],
            origin: Some("ROUTINE".into()),
            parent_task_id: None,
            dedupe: Some(false),
        },
    )
    .await?;
    let body = crate::api::tasks::DispatchBody {
        policy: None,
        manual_agent: Some(agent_id.to_string()),
        manual_model: None,
        manual_effort: None,
        execution_mode: None,
        prompt_append: None,
    };
    if let Err(error) = crate::api::tasks::dispatch_task_internal(state, &task.id, body, None).await
    {
        tracing::warn!(
            agent_id,
            task_id = %task.id,
            error = %error,
            "heartbeat task created without dispatch"
        );
    }
    tasks::get(&state.db, &task.id).await.map_err(Into::into)
}

pub fn start_heartbeat(state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            if let Err(error) = heartbeat_tick(&state).await {
                tracing::warn!(error = %error, "agent heartbeat tick failed");
            }
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }
    });
}

async fn heartbeat_tick(state: &Arc<AppState>) -> AppResult<()> {
    for profile in due_heartbeats(&state.db).await? {
        // Claim the slot before doing work so a slow run cannot double-fire.
        mark_heartbeat(
            &state.db,
            &profile.workspace_id,
            &profile.agent_id,
            profile.heartbeat_minutes,
        )
        .await?;
        if let Err(error) = run_heartbeat(state, &profile.workspace_id, &profile.agent_id).await {
            tracing::warn!(
                workspace_id = %profile.workspace_id,
                agent_id = %profile.agent_id,
                error = %error,
                "heartbeat skipped"
            );
        }
    }
    Ok(())
}
