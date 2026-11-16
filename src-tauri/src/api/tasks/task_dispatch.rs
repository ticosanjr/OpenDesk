use crate::{
    app_state::AppState,
    db::{self, budget, queue::EnqueueInput, router as router_db, tasks, workspaces},
    dsh_router::{self, PackContext, RouterInput},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
};
use std::sync::Arc;

use super::{DispatchBody, validate_model_ref};

pub(super) fn concurrency_group(agent: &str) -> String {
    let agent = agent.to_lowercase();
    if agent.contains("local") || agent.contains("llm") {
        "local_llm_heavy".into()
    } else if agent.contains("explore") || agent.contains("web") {
        "web_scraper".into()
    } else {
        "light_api".into()
    }
}

pub(crate) async fn dispatch_task_internal(
    state: &Arc<AppState>,
    id: &str,
    body: DispatchBody,
    approved_plan: Option<&str>,
) -> AppResult<serde_json::Value> {
    dispatch_task_with_session(state, id, body, approved_plan, None).await
}

pub(crate) async fn dispatch_task_with_session(
    state: &Arc<AppState>,
    id: &str,
    body: DispatchBody,
    approved_plan: Option<&str>,
    session_id: Option<&str>,
) -> AppResult<serde_json::Value> {
    validate_model_ref(body.manual_model.as_deref())?;
    let task = tasks::get(&state.db, id).await?;
    let workspace = workspaces::get(&state.db, &task.workspace_id).await?;
    if let Some(agent) = body.manual_agent.as_deref() {
        crate::governance::check_budget(&state.db, &task.workspace_id, agent).await?;
    }
    crate::api::engine::require_engine_strict(state).await?;
    let decision = plan_decision(state, &task, &workspace, &body, approved_plan).await?;
    crate::governance::check_budget(&state.db, &task.workspace_id, &decision.selected_agent)
        .await?;
    let extra = body
        .prompt_append
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if extra.is_some_and(|value| value.chars().count() > 8_000) {
        return Err(AppError::BadRequest(
            "instruções desta execução excedem 8000 caracteres".into(),
        ));
    }
    let job = enqueue_decision_with_session(
        state,
        &task,
        &workspace,
        &decision,
        approved_plan,
        session_id,
        extra,
    )
    .await?;
    Ok(serde_json::json!({
        "decision": decision,
        "job": job,
    }))
}

pub(super) fn model_key(model: &crate::opencode_client::rest_client::ModelDto) -> String {
    let provider = model.provider_id.clone().unwrap_or_default();
    let id = model.model_id.clone().unwrap_or_else(|| model.id.clone());
    format!("{provider}/{id}")
}

pub(super) fn base_model(model: &str) -> String {
    let (provider, id) = db::model_health::split_model(model);
    format!("{provider}/{id}")
}

pub(crate) async fn plan_decision(
    state: &AppState,
    task: &tasks::Task,
    workspace: &workspaces::Workspace,
    body: &DispatchBody,
    approved_plan: Option<&str>,
) -> AppResult<dsh_router::RouterDecision> {
    let rules = router_db::list(&state.db, &task.workspace_id).await?;
    let raw_models = state
        .daemon
        .models(Some(&workspace.root_path))
        .await
        .unwrap_or_default();
    let cooling = db::model_health::cooling(&state.db, 24)
        .await
        .unwrap_or_default();
    let filtered = raw_models
        .iter()
        .filter(|model| !cooling.iter().any(|blocked| blocked == &model_key(model)))
        .cloned()
        .collect::<Vec<_>>();
    // If every model is cooling down, keep the full list so the user can still
    // see the real provider error instead of an empty router.
    let available_models = if filtered.is_empty() {
        raw_models
    } else {
        filtered
    };
    // A model chosen by the user is never replaced by the router, even when it
    // is cooling down: failing with the provider error is clearer than silently
    // running on a different model.
    let task_model = if task.model_override {
        task.preferred_model.as_deref()
    } else {
        None
    };
    let budget_status = budget::status(&state.db).await.ok();
    let policy = body
        .policy
        .clone()
        .unwrap_or_else(|| workspace.default_router_policy.clone());
    let execution_mode = body
        .execution_mode
        .clone()
        .unwrap_or_else(|| task.execution_mode.clone());
    let tags = task.tags();
    let router_models = router_db::models(&state.db, &task.workspace_id)
        .await
        .unwrap_or_default();
    let default_harness = crate::adapters::default_harness(&state.db).await?;
    let input = RouterInput {
        workspace_id: &task.workspace_id,
        workspace_root: std::path::Path::new(&workspace.root_path),
        title: &task.title,
        prompt: &task.description,
        execution_mode: &execution_mode,
        priority: task.priority,
        tags: &tags,
        manual_agent: body.manual_agent.as_deref().or(if task.agent_override {
            task.assigned_agent.as_deref()
        } else {
            None
        }),
        manual_model: body.manual_model.as_deref().or(task_model),
        manual_effort: body.manual_effort.as_deref(),
        manual_harness: task.preferred_harness.as_deref(),
        default_harness: Some(&default_harness),
        policy: &policy,
        rules: &rules,
        available_models: &available_models,
        default_model: router_models.default_model.as_deref(),
        manual_default_model: router_models.manual_model.as_deref(),
        approved_plan,
        output_directory: task.output_dir_path.as_deref().map(std::path::Path::new),
        budget_daily_pct: budget_status
            .as_ref()
            .map(|status| status.daily_pct)
            .unwrap_or(0.0),
    };
    let mut decision = dsh_router::classify(&input);
    // Quick work tasks can use the lean OpenDesk agent, but only when there is
    // no explicit choice (task/agent dialog/rule) and the agent exists.
    if task.care_level == "quick"
        && decision.phase == "build"
        && decision.origin != "RULE"
        && !task.agent_override
        && body.manual_agent.is_none()
        && opendesk_agent_available(std::path::Path::new(&workspace.root_path))
    {
        decision.selected_agent = "opendesk".into();
    }
    Ok(decision)
}

fn opendesk_agent_available(workspace_root: &std::path::Path) -> bool {
    let paths = crate::config_sync::paths::StudioPaths::new(Some(workspace_root.to_path_buf()));
    ["workspace", "global"].iter().any(|scope| {
        paths
            .agents_dir(scope)
            .map(|dir| dir.join("opendesk.md").is_file())
            .unwrap_or(false)
    })
}

#[allow(dead_code)]
pub(crate) fn care_instructions(care: &str) -> &'static str {
    match care {
        "quick" => {
            "\n\n## Nível de cuidado: rápido\n\nSeja direto. Não faça plano formal nem auditoria. Execute o necessário, faça no máximo uma validação objetiva e entregue o resultado. Evite exploração extensa de arquivos."
        }
        "careful" => {
            "\n\n## Nível de cuidado: cuidadoso\n\nRevise premissas, valide o resultado e registre riscos ou pendências. Prefira precisão a velocidade."
        }
        _ => {
            "\n\n## Nível de cuidado: padrão\n\nUse um checklist curto, execute o necessário e faça uma validação. Não transforme tarefas simples em um plano longo."
        }
    }
}

pub(super) async fn enqueue_decision(
    state: &AppState,
    task: &tasks::Task,
    workspace: &workspaces::Workspace,
    decision: &dsh_router::RouterDecision,
    approved_plan: Option<&str>,
) -> AppResult<crate::db::queue::QueueJob> {
    enqueue_decision_with_session(state, task, workspace, decision, approved_plan, None, None).await
}

fn append_instruction(
    prompt: String,
    extra: Option<&str>,
    templates: &crate::dsh_router::prompt_templates::PromptTemplates,
) -> String {
    match extra.map(str::trim).filter(|value| !value.is_empty()) {
        Some(extra) => {
            let block = crate::dsh_router::prompt_templates::render(
                &templates.execution_instruction,
                &[("instruction", extra)],
            );
            format!("{prompt}\n{block}\n")
        }
        None => prompt,
    }
}

pub(crate) async fn enqueue_decision_with_session(
    state: &AppState,
    task: &tasks::Task,
    workspace: &workspaces::Workspace,
    decision: &dsh_router::RouterDecision,
    approved_plan: Option<&str>,
    session_id: Option<&str>,
    extra_instruction: Option<&str>,
) -> AppResult<crate::db::queue::QueueJob> {
    std::fs::create_dir_all(&decision.output_directory).map_err(|error| {
        AppError::Internal(format!("could not create output directory: {error}"))
    })?;
    tasks::set_output_dir(&state.db, &task.id, &decision.output_directory).await?;

    let tags = task.tags();
    let care = task.care_level.as_str();
    let selected_agent = decision.selected_agent.clone();
    let needs_browser = crate::browse::ensure_for_task(
        state,
        &task.workspace_id,
        &task.title,
        &task.description,
        &tags,
    )
    .await?;
    let personal_context =
        crate::personal_context::render_body(&state.db, &task.workspace_id).await?;
    let memory_context = crate::db::memory::render_context(
        &state.db,
        &task.workspace_id,
        &selected_agent,
        &format!("{} {}", task.title, task.description),
        8,
    )
    .await
    .unwrap_or_default();
    let session_packet = crate::session_context::render_and_persist(&state.db, &task.id)
        .await
        .unwrap_or_default();
    let templates = crate::dsh_router::prompt_templates::PromptTemplates::load(
        &state.db,
        Some(&task.workspace_id),
    )
    .await?;
    let prompt = dsh_router::pack(
        &PackContext {
            workspace_root: std::path::Path::new(&workspace.root_path),
            output_directory: &decision.output_directory,
            task_id: &task.id,
            task_title: &task.title,
            task_description: &task.description,
            tags: &tags,
            priority: task.priority,
            phase: &decision.phase,
            plan: approved_plan.or(task.plan_markdown.as_deref()),
            agent: &selected_agent,
            care_level: care,
            needs_browser,
            personal_context: &personal_context,
            structured_memory: &memory_context,
            session_packet: &session_packet,
        },
        &templates,
    );
    let prompt = append_instruction(prompt, extra_instruction, &templates);

    tasks::move_to(&state.db, &task.id, "TODO", None).await?;
    tasks::set_phase(&state.db, &task.id, Some(&decision.phase)).await?;

    let job = state
        .queue
        .enqueue(EnqueueInput {
            workspace_id: task.workspace_id.clone(),
            task_id: Some(task.id.clone()),
            schedule_id: None,
            session_id: session_id.map(str::to_string),
            concurrency_group: Some(concurrency_group(&selected_agent)),
            prompt_payload: prompt,
            resolved_mode: decision.phase.clone(),
            resolved_agent: selected_agent,
            resolved_model: decision.selected_model.clone(),
            resolved_effort: decision.effort.clone(),
            resolved_harness: decision.harness.clone(),
            job_kind: "task".into(),
            scheduled_for: None,
            output_dir: Some(decision.output_directory.clone()),
            priority: task.priority,
            max_retries: 1,
        })
        .await?;

    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: "TODO".into(),
        phase_status: Some("queued".into()),
    });

    Ok(job)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_one_off_execution_instructions() {
        let prompt = append_instruction(
            "base".into(),
            Some("  /resumir foco  "),
            &crate::dsh_router::prompt_templates::PromptTemplates::default(),
        );
        assert!(prompt.starts_with("base"));
        assert!(prompt.contains("## Instruções desta execução"));
        assert!(prompt.contains("/resumir foco"));
        assert_eq!(
            append_instruction(
                "base".into(),
                Some("   "),
                &crate::dsh_router::prompt_templates::PromptTemplates::default()
            ),
            "base"
        );
        assert_eq!(
            append_instruction(
                "base".into(),
                None,
                &crate::dsh_router::prompt_templates::PromptTemplates::default()
            ),
            "base"
        );
    }

    #[test]
    fn care_instructions_match_the_level() {
        assert!(care_instructions("quick").contains("Não faça plano formal"));
        assert!(care_instructions("careful").contains("Revise premissas"));
        assert!(care_instructions("standard").contains("checklist curto"));
    }

    #[test]
    fn opendesk_agent_is_detected_in_the_workspace() {
        let temp = tempfile::TempDir::new().expect("temp");
        assert!(!opendesk_agent_available(temp.path()));
        let agents = temp.path().join(".opencode/agents");
        std::fs::create_dir_all(&agents).expect("agents");
        std::fs::write(agents.join("opendesk.md"), "---\nname: OpenDesk\n---").expect("agent");
        assert!(opendesk_agent_available(temp.path()));
    }
}
