use crate::{
    db::{self, tasks},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
    recipe_context::RecipeContext,
    recipes::{
        hitl::{self, HitlOutcome, HitlResolution},
        parser::{self, FlowItem, Recipe, Step},
        store,
    },
};
use chrono::Utc;
use serde_json::json;
use std::time::Duration;

pub async fn start(
    context: RecipeContext,
    workspace_id: &str,
    recipe: Recipe,
    trigger: serde_json::Value,
    dry_run: bool,
) -> AppResult<store::RecipeRun> {
    let index = store::get(&context.pool, workspace_id, &recipe.id).await?;
    if !index.is_enabled || !recipe.enabled {
        return Err(AppError::BadRequest("receita desabilitada".into()));
    }
    let workspace = db::workspaces::get(&context.pool, &index.workspace_id).await?;
    let output = crate::dsh_router::output_directory(
        std::path::Path::new(&workspace.root_path),
        &format!(
            "{} {}",
            recipe.name,
            uuid::Uuid::now_v7()
                .simple()
                .to_string()
                .get(..8)
                .unwrap_or("run")
        ),
    );
    std::fs::create_dir_all(&output)?;
    let output = output.to_string_lossy().to_string();

    if dry_run {
        let run = store::create_run(
            &context.pool,
            &recipe,
            &index.workspace_id,
            None,
            &trigger,
            &output,
            true,
        )
        .await?;
        let context_clone = context.clone();
        let recipe_clone = recipe.clone();
        let run_clone = run.clone();
        tokio::spawn(async move {
            for step in recipe_clone.all_steps() {
                let _ = store::update_run(
                    &context_clone.pool,
                    &run_clone.id,
                    "RUNNING_STEP",
                    Some(&step.id),
                    None,
                    None,
                    false,
                )
                .await;
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            let _ = store::update_run(
                &context_clone.pool,
                &run_clone.id,
                "COMPLETED",
                None,
                Some(&json!({"dry_run": true})),
                None,
                true,
            )
            .await;
            context_clone
                .events
                .publish(OpenDeskWsEvent::RecipeRunUpdated {
                    run_id: run_clone.id,
                    recipe_id: recipe_clone.id,
                    workspace_id: index.workspace_id,
                    status: "COMPLETED".into(),
                });
        });
        return Ok(run);
    }

    let task = tasks::create(
        &context.pool,
        &index.workspace_id,
        tasks::CreateTaskInput {
            title: format!("{} · receita", recipe.name),
            description: recipe
                .description
                .clone()
                .unwrap_or_else(|| format!("Receita {}", recipe.id)),
            execution_mode: Some("BUILD_DIRECT".into()),
            priority: Some(2),
            assigned_agent: None,
            preferred_model: None,
            preferred_harness: None,
            effort_level: None,
            care_level: Some("standard".into()),
            notify_email: None,
            export_to_gdocs: false,
            tags: vec![format!("recipe:{}", recipe.id)],
            origin: Some("RECIPE".into()),
            parent_task_id: None,
            dedupe: None,
        },
    )
    .await?;
    if task.deduplicated
        && let Some(existing) = store::latest_run_for_task(&context.pool, &task.id).await?
    {
        tracing::info!(
            recipe_id = %recipe.id,
            run_id = %existing.id,
            "recipe run deduplicated"
        );
        return Ok(existing);
    }
    tasks::set_output_dir(&context.pool, &task.id, &output).await?;
    let run = store::create_run(
        &context.pool,
        &recipe,
        &index.workspace_id,
        Some(&task.id),
        &trigger,
        &output,
        false,
    )
    .await?;
    store::sync_run_nodes(&context.pool, &run.id, &recipe.flow).await?;
    let context_clone = context.clone();
    let run_clone = run.clone();
    let recipe_clone = recipe.clone();
    tokio::spawn(async move {
        if let Err(error) = execute(
            &context_clone,
            &recipe_clone,
            &run_clone,
            &trigger,
            context_clone.dispatcher(),
        )
        .await
        {
            tracing::warn!(run = %run_clone.id, error = %error, "recipe run failed");
            let message = format!("{error:#}");
            let _ = store::fail_running_nodes(&context_clone.pool, &run_clone.id, &message).await;
            let _ = store::update_run(
                &context_clone.pool,
                &run_clone.id,
                "FAILED",
                None,
                None,
                Some(&message),
                true,
            )
            .await;
            if let Some(task_id) = &run_clone.task_id {
                let _ = tasks::set_phase_status(
                    &context_clone.pool,
                    task_id,
                    Some("failed"),
                    Some("BLOCKED"),
                )
                .await;
                let _ = tasks::set_blocked_reason(
                    &context_clone.pool,
                    task_id,
                    Some(&format!("{error:#}")),
                )
                .await;
            }
            mark_routine(&context_clone, &trigger, "FAILED").await;
            context_clone
                .events
                .publish(OpenDeskWsEvent::RecipeRunUpdated {
                    run_id: run_clone.id,
                    recipe_id: recipe_clone.id,
                    workspace_id: run_clone.workspace_id,
                    status: "FAILED".into(),
                });
        }
    });
    Ok(run)
}

/// Restarts a durable run after boot/crash/retry. Existing DONE nodes are kept.
pub async fn resume(
    context: RecipeContext,
    recipe: Recipe,
    run: store::RecipeRun,
) -> AppResult<()> {
    let trigger: serde_json::Value =
        serde_json::from_str(&run.trigger_context_json).unwrap_or_else(|_| json!({}));
    store::sync_run_nodes(&context.pool, &run.id, &recipe.flow).await?;
    let _ = store::update_run(
        &context.pool,
        &run.id,
        "RUNNING_STEP",
        None,
        None,
        None,
        false,
    )
    .await;
    let context_clone = context.clone();
    let run_clone = run.clone();
    let recipe_clone = recipe.clone();
    tokio::spawn(async move {
        if let Err(error) = execute(
            &context_clone,
            &recipe_clone,
            &run_clone,
            &trigger,
            context_clone.dispatcher(),
        )
        .await
        {
            let message = format!("{error:#}");
            let _ = store::fail_running_nodes(&context_clone.pool, &run_clone.id, &message).await;
            let _ = store::update_run(
                &context_clone.pool,
                &run_clone.id,
                "FAILED",
                None,
                None,
                Some(&message),
                true,
            )
            .await;
            if let Some(task_id) = &run_clone.task_id {
                let _ = tasks::set_phase_status(
                    &context_clone.pool,
                    task_id,
                    Some("failed"),
                    Some("BLOCKED"),
                )
                .await;
                let _ =
                    tasks::set_blocked_reason(&context_clone.pool, task_id, Some(&message)).await;
            }
            context_clone
                .events
                .publish(OpenDeskWsEvent::RecipeRunUpdated {
                    run_id: run_clone.id,
                    recipe_id: recipe_clone.id,
                    workspace_id: run_clone.workspace_id,
                    status: "FAILED".into(),
                });
        }
    });
    Ok(())
}

async fn execute(
    context: &RecipeContext,
    recipe: &Recipe,
    run: &store::RecipeRun,
    trigger: &serde_json::Value,
    dispatcher: &dyn crate::recipe_dispatch::RecipeStepDispatcher,
) -> AppResult<()> {
    if recipe.has_dag() {
        return crate::recipes::dag::execute(context, recipe, run, trigger, dispatcher).await;
    }
    let task_id = run
        .task_id
        .clone()
        .ok_or_else(|| AppError::BadRequest("recipe run sem task".into()))?;
    let run_vars = json!({
        "id": run.id,
        "output_dir": run.output_dir_path,
        "date": Utc::now().format("%Y-%m-%d").to_string(),
    });
    let mut outputs: serde_json::Value =
        serde_json::from_str(&run.step_outputs_json).unwrap_or_else(|_| serde_json::json!({}));
    let mut session_id: Option<String> = None;
    let routine_id = trigger
        .get("routine_id")
        .or_else(|| trigger.get("schedule_id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let mut feedback_by_step: std::collections::HashMap<String, String> = Default::default();
    let mut review_rounds = 0usize;
    let flow = recipe.flow.clone();
    store::sync_run_nodes(&context.pool, &run.id, &flow).await?;
    let nodes = store::list_run_nodes(&context.pool, &run.id).await?;
    let mut index = nodes
        .iter()
        .find(|node| !matches!(node.status.as_str(), "DONE" | "SKIPPED"))
        .map(|node| node.node_index as usize)
        .unwrap_or(flow.len());

    while index < flow.len() {
        if store::is_cancel_requested(&context.pool, &run.id).await? {
            finish(
                context,
                run,
                "REJECTED",
                "cancelado pelo usuário",
                routine_id.as_deref(),
            )
            .await?;
            return Ok(());
        }
        store::mark_node_running(&context.pool, &run.id, index).await?;
        match &flow[index] {
            FlowItem::Step(step) => {
                let should_run = step
                    .when
                    .as_deref()
                    .map(|condition| parser::evaluate_when(condition, trigger, &run_vars, &outputs))
                    .transpose()?
                    .unwrap_or(true);
                if !should_run {
                    if let Some(previous) = flow[..index].iter().rev().find_map(|item| match item {
                        FlowItem::Step(step) => Some(step.id.clone()),
                        _ => None,
                    }) {
                        outputs[&step.id] = serde_json::json!({
                            "skipped": true,
                            "reason": step.when,
                            "after": previous,
                        });
                    } else {
                        outputs[&step.id] = serde_json::json!({
                            "skipped": true,
                            "reason": step.when,
                        });
                    }
                    store::mark_node_skipped(&context.pool, &run.id, index).await?;
                    index += 1;
                    continue;
                }
                let feedback = feedback_by_step.remove(&step.id);
                if let Err(error) = execute_step(
                    context,
                    dispatcher,
                    run,
                    &task_id,
                    step,
                    trigger,
                    &run_vars,
                    &mut outputs,
                    &mut session_id,
                    feedback,
                    routine_id.as_deref(),
                )
                .await
                {
                    if store::is_cancel_requested(&context.pool, &run.id).await? {
                        finish(
                            context,
                            run,
                            "REJECTED",
                            "cancelado pelo usuário",
                            routine_id.as_deref(),
                        )
                        .await?;
                        return Ok(());
                    }
                    return Err(error);
                }
                store::mark_node_done(
                    &context.pool,
                    &run.id,
                    index,
                    outputs.get(&step.id),
                    session_id.as_deref(),
                )
                .await?;
                index += 1;
            }
            FlowItem::Hitl(checkpoint) => {
                let hitl_index = index;
                let mut checkpoint = checkpoint.clone();
                checkpoint.id = parser::checkpoint_id(&checkpoint);
                let current_step = flow[..index].iter().rev().find_map(|item| match item {
                    FlowItem::Step(step) => Some(step.id.clone()),
                    _ => None,
                });
                store::update_run(
                    &context.pool,
                    &run.id,
                    "WAITING_HITL",
                    current_step.as_deref(),
                    Some(&outputs),
                    None,
                    false,
                )
                .await?;
                store::mark_node_waiting(&context.pool, &run.id, index).await?;
                let _ = tasks::move_to(&context.pool, &task_id, "IN_REVIEW", None).await;
                let _ = tasks::set_phase_status(
                    &context.pool,
                    &task_id,
                    Some("waiting_hitl"),
                    Some("IN_REVIEW"),
                )
                .await;
                let message =
                    parser::render(&checkpoint.prompt_template, trigger, &run_vars, &outputs)?;
                let attachments = checkpoint
                    .attach_files
                    .iter()
                    .map(|path| parser::render(path, trigger, &run_vars, &outputs))
                    .collect::<AppResult<Vec<_>>>()?;
                let next_step = flow.get(index + 1).and_then(|item| match item {
                    FlowItem::Step(step) => Some(step.name.as_str()),
                    FlowItem::Hitl(next) => Some(next.id.as_str()),
                    FlowItem::Sink(sink) => sink.name.as_deref(),
                    _ => None,
                });
                let token = match store::pending_hitl_for_checkpoint(
                    &context.pool,
                    &run.id,
                    &checkpoint.id,
                )
                .await?
                {
                    Some(existing) => existing.token,
                    None => {
                        hitl::request(
                            context,
                            run,
                            &checkpoint,
                            &recipe.name,
                            &message,
                            next_step,
                            &attachments,
                        )
                        .await?
                    }
                };
                context.events.publish(OpenDeskWsEvent::HitlRequested {
                    token: token.clone(),
                    run_id: run.id.clone(),
                    workspace_id: run.workspace_id.clone(),
                    checkpoint_id: checkpoint.id.clone(),
                    interaction_type: checkpoint.interaction_type.clone(),
                });
                let outcome = hitl::wait(context, &token, checkpoint.timeout_minutes).await?;
                match outcome {
                    HitlOutcome::Resolved(HitlResolution::Approved) => {
                        context.events.publish(OpenDeskWsEvent::HitlResolved {
                            token,
                            status: "APPROVED".into(),
                        });
                        index += 1;
                    }
                    HitlOutcome::Resolved(HitlResolution::Rejected { reason }) => {
                        finish(
                            context,
                            run,
                            "REJECTED",
                            reason.as_deref().unwrap_or("rejeitado pelo usuário"),
                            routine_id.as_deref(),
                        )
                        .await?;
                        return Ok(());
                    }
                    HitlOutcome::Resolved(HitlResolution::FeedbackProvided {
                        feedback_text,
                        attached_files,
                    }) => {
                        if let Some(path) = attached_files.first() {
                            replace_attachment(path, &attachments)?;
                            index += 1;
                            continue;
                        }
                        review_rounds += 1;
                        if review_rounds > 5 {
                            finish(
                                context,
                                run,
                                "REJECTED",
                                "limite de revisões atingido",
                                routine_id.as_deref(),
                            )
                            .await?;
                            return Ok(());
                        }
                        let Some((previous_index, previous_step)) = previous_step(&flow, index)
                        else {
                            finish(
                                context,
                                run,
                                "REJECTED",
                                "checkpoint sem step anterior para refazer",
                                routine_id.as_deref(),
                            )
                            .await?;
                            return Ok(());
                        };
                        feedback_by_step.insert(previous_step.id.clone(), feedback_text);
                        index = previous_index;
                    }
                    HitlOutcome::Resolved(HitlResolution::ChoiceSelected {
                        option_value, ..
                    }) => {
                        review_rounds += 1;
                        if review_rounds > 5 {
                            finish(
                                context,
                                run,
                                "REJECTED",
                                "limite de revisões atingido",
                                routine_id.as_deref(),
                            )
                            .await?;
                            return Ok(());
                        }
                        let Some((previous_index, previous_step)) = previous_step(&flow, index)
                        else {
                            finish(
                                context,
                                run,
                                "REJECTED",
                                "checkpoint sem step anterior para refazer",
                                routine_id.as_deref(),
                            )
                            .await?;
                            return Ok(());
                        };
                        feedback_by_step.insert(
                            previous_step.id.clone(),
                            format!("Opção escolhida: {option_value}"),
                        );
                        index = previous_index;
                    }
                    HitlOutcome::Resolved(HitlResolution::ArtifactEdited { path }) => {
                        replace_attachment(&path, &attachments)?;
                        index += 1;
                    }
                    HitlOutcome::Expired => {
                        if checkpoint.on_timeout == "AUTO_APPROVE" {
                            index += 1;
                        } else {
                            finish(
                                context,
                                run,
                                "TIMED_OUT",
                                "timeout do checkpoint HITL",
                                routine_id.as_deref(),
                            )
                            .await?;
                            return Ok(());
                        }
                    }
                    HitlOutcome::Cancelled => {
                        finish(
                            context,
                            run,
                            "REJECTED",
                            "cancelado pelo usuário",
                            routine_id.as_deref(),
                        )
                        .await?;
                        return Ok(());
                    }
                }
                if index == hitl_index + 1 {
                    let _ =
                        store::mark_node_done(&context.pool, &run.id, hitl_index, None, None).await;
                } else if index < hitl_index {
                    // Feedback/choice re-runs the previous step and this HITL.
                    let _ = store::reset_node(&context.pool, &run.id, hitl_index).await;
                    let _ = store::reset_node(&context.pool, &run.id, index).await;
                }
            }
            FlowItem::Sink(sink) => {
                store::update_run(
                    &context.pool,
                    &run.id,
                    "RUNNING_SINKS",
                    None,
                    Some(&outputs),
                    None,
                    false,
                )
                .await?;
                context
                    .sinks()
                    .execute(context, sink, run, trigger, &outputs)
                    .await?;
                store::mark_node_done(&context.pool, &run.id, index, Some(&outputs), None).await?;
                index += 1;
            }
            FlowItem::Branch(_)
            | FlowItem::Parallel(_)
            | FlowItem::Map(_)
            | FlowItem::Recipe(_)
            | FlowItem::Loop(_)
            | FlowItem::Switch(_) => {
                unreachable!("DAG flows are routed to recipes::dag")
            }
        }
    }

    let _ = tasks::move_to(&context.pool, &task_id, "DONE", None).await;
    let _ = tasks::set_phase_status(&context.pool, &task_id, Some("done"), Some("DONE")).await;
    store::update_run(
        &context.pool,
        &run.id,
        "COMPLETED",
        None,
        Some(&outputs),
        None,
        true,
    )
    .await?;
    mark_routine(context, trigger, "SUCCESS").await;
    context.events.publish(OpenDeskWsEvent::RecipeRunUpdated {
        run_id: run.id.clone(),
        recipe_id: recipe.id.clone(),
        workspace_id: run.workspace_id.clone(),
        status: "COMPLETED".into(),
    });
    Ok(())
}

fn ensure_step_finished(step_id: &str, status: &str) -> AppResult<()> {
    match status {
        "SUCCESS" => Ok(()),
        "FAILED" => Err(AppError::BadRequest(format!(
            "step {step_id} falhou; a receita foi interrompida"
        ))),
        "CANCELLED" => Err(AppError::BadRequest(format!(
            "step {step_id} foi cancelado; a receita foi interrompida"
        ))),
        other => Err(AppError::BadRequest(format!(
            "step {step_id} terminou com status inesperado '{other}'"
        ))),
    }
}

fn previous_step(flow: &[FlowItem], index: usize) -> Option<(usize, &Step)> {
    flow[..index]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(position, item)| match item {
            FlowItem::Step(step) => Some((position, step)),
            _ => None,
        })
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn execute_step(
    context: &RecipeContext,
    dispatcher: &dyn crate::recipe_dispatch::RecipeStepDispatcher,
    run: &store::RecipeRun,
    task_id: &str,
    step: &Step,
    trigger: &serde_json::Value,
    run_vars: &serde_json::Value,
    outputs: &mut serde_json::Value,
    session_id: &mut Option<String>,
    feedback: Option<String>,
    routine_id: Option<&str>,
) -> AppResult<()> {
    store::update_run(
        &context.pool,
        &run.id,
        "RUNNING_STEP",
        Some(&step.id),
        Some(outputs),
        None,
        false,
    )
    .await?;
    if let Some(schema) = step.input_schema.as_ref() {
        let input = serde_json::json!({
            "trigger": trigger,
            "run": run_vars,
            "steps": outputs,
        });
        let validator = jsonschema::validator_for(schema)
            .map_err(|error| AppError::BadRequest(format!("input_schema inválido: {error}")))?;
        validator.validate(&input).map_err(|error| {
            AppError::BadRequest(format!("input do step {} fora do schema: {error}", step.id))
        })?;
    }

    let templates = crate::dsh_router::prompt_templates::PromptTemplates::load(
        &context.pool,
        Some(&run.workspace_id),
    )
    .await
    .unwrap_or_default();
    let base_prompt = parser::render(&step.prompt, trigger, run_vars, outputs)?;
    let schema_block = step.output_schema.as_ref().map(|schema| {
        format!(
            "\n\n## Contrato de saída (JSON Schema)\nResponda APENAS com um objeto JSON válido para este schema, sem texto fora do JSON:\n```json\n{}\n```",
            serde_json::to_string_pretty(schema).unwrap_or_default()
        )
    });
    let mut attempt = 0usize;
    let mut correction = feedback;
    loop {
        attempt += 1;
        let mut prompt = base_prompt.clone();
        if let Some(feedback) = correction.as_deref() {
            let block = crate::dsh_router::prompt_templates::render(
                &templates.recipe_feedback,
                &[("feedback", feedback)],
            );
            prompt.push_str("\n\n");
            prompt.push_str(&block);
        }
        if let Some(block) = schema_block.as_deref() {
            prompt.push_str(block);
        }
        tasks::update(
            &context.pool,
            task_id,
            tasks::UpdateTaskInput {
                description: Some(prompt.clone()),
                ..Default::default()
            },
        )
        .await?;
        let started = Utc::now().to_rfc3339();
        crate::recipe_dispatch::RecipeStepDispatcher::dispatch(
            dispatcher,
            task_id,
            crate::recipe_dispatch::RecipeStepRequest {
                agent: step
                    .agent
                    .clone()
                    .map(|value| value.trim_start_matches('@').to_string()),
                model: step.model.clone(),
                effort: step.effort.clone(),
                execution_mode: Some(mode_for_step(step)),
                prompt_append: None,
            },
            session_id.as_deref(),
        )
        .await?;
        let run_result =
            wait_for_run(context, &run.workspace_id, task_id, &started, &run.id).await?;
        if let Some(routine_id) = routine_id {
            let _ = db::runs::set_schedule_id(&context.pool, &run_result.id, routine_id).await;
        }
        ensure_step_finished(&step.id, &run_result.status)?;
        *session_id = run_result.session_id.clone();
        let summary = run_result.summary_response.clone().unwrap_or_default();
        let structured = match step.output_schema.as_ref() {
            Some(schema) => match parser::validate_output(schema, &summary) {
                Ok(value) => Some(value),
                Err(error) if attempt < 2 => {
                    correction = Some(format!(
                        "A resposta anterior não validou o contrato: {error}. Responda novamente apenas com JSON válido."
                    ));
                    continue;
                }
                Err(error) => return Err(error),
            },
            None => None,
        };
        let artifacts = db::artifacts::list_by_run(&context.pool, &run_result.id, 200)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|artifact| artifact.file_path)
            .collect::<Vec<_>>();
        outputs[&step.id] = serde_json::json!({
            "run_id": run_result.id,
            "summary": summary,
            "artifacts": artifacts,
            "structured": structured,
            "attempts": attempt,
        });
        store::update_run(
            &context.pool,
            &run.id,
            "RUNNING_STEP",
            Some(&step.id),
            Some(outputs),
            None,
            false,
        )
        .await?;
        return Ok(());
    }
}

async fn wait_for_run(
    context: &RecipeContext,
    workspace_id: &str,
    task_id: &str,
    started_after: &str,
    recipe_run_id: &str,
) -> AppResult<db::runs::TaskRun> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2 * 60 * 60);
    loop {
        if store::is_cancel_requested(&context.pool, recipe_run_id).await? {
            return Err(AppError::BadRequest("cancelado pelo usuário".into()));
        }
        if let Ok(runs) =
            db::runs::list_with_limit(&context.pool, workspace_id, Some(task_id), 5).await
            && let Some(run) = runs
                .into_iter()
                .find(|run| run.started_at.as_str() >= started_after)
            && matches!(run.status.as_str(), "SUCCESS" | "FAILED" | "CANCELLED")
        {
            return Ok(run);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(AppError::Internal(
                "timeout aguardando step da receita".into(),
            ));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

pub(super) fn replace_attachment(source: &str, targets: &[String]) -> AppResult<()> {
    let target = targets
        .first()
        .ok_or_else(|| AppError::BadRequest("checkpoint sem arquivo de destino".into()))?;
    let bytes = std::fs::read(source)?;
    if let Some(parent) = std::path::Path::new(target).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(target, bytes)?;
    Ok(())
}

pub(super) async fn finish(
    context: &RecipeContext,
    run: &store::RecipeRun,
    status: &str,
    message: &str,
    routine_id: Option<&str>,
) -> AppResult<()> {
    let _ = store::fail_running_nodes(&context.pool, &run.id, message).await;
    store::update_run(
        &context.pool,
        &run.id,
        status,
        None,
        None,
        Some(message),
        true,
    )
    .await?;
    if let Some(task_id) = &run.task_id {
        let _ =
            tasks::set_phase_status(&context.pool, task_id, Some("failed"), Some("BLOCKED")).await;
        let _ = tasks::set_blocked_reason(&context.pool, task_id, Some(message)).await;
    }
    if let Some(routine_id) = routine_id {
        mark_routine_by_id(context, routine_id, status).await;
    }
    context.events.publish(OpenDeskWsEvent::RecipeRunUpdated {
        run_id: run.id.clone(),
        recipe_id: run.recipe_id.clone(),
        workspace_id: run.workspace_id.clone(),
        status: status.to_string(),
    });
    Ok(())
}

pub(super) async fn mark_routine(
    context: &RecipeContext,
    trigger: &serde_json::Value,
    status: &str,
) {
    if let Some(routine_id) = trigger
        .get("routine_id")
        .or_else(|| trigger.get("schedule_id"))
        .and_then(serde_json::Value::as_str)
    {
        mark_routine_by_id(context, routine_id, status).await;
    }
}

async fn mark_routine_by_id(context: &RecipeContext, routine_id: &str, status: &str) {
    if let Ok(schedule) = crate::db::schedules::get(&context.pool, routine_id).await {
        let _ = crate::db::schedules::mark_run(
            &context.pool,
            &schedule.id,
            status,
            &Utc::now().to_rfc3339(),
        )
        .await;
        context.events.publish(OpenDeskWsEvent::ScheduleUpdated {
            schedule_id: schedule.id,
            workspace_id: schedule.workspace_id,
        });
    }
}

fn mode_for_step(step: &Step) -> String {
    match step.mode.as_str() {
        "plan" => "PLAN_ONLY".into(),
        _ => "BUILD_DIRECT".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recipes::parser::Step;

    #[test]
    fn aborta_receita_quando_step_falha() {
        assert!(ensure_step_finished("s1", "SUCCESS").is_ok());
        assert!(ensure_step_finished("s1", "FAILED").is_err());
        assert!(ensure_step_finished("s1", "CANCELLED").is_err());
        assert!(ensure_step_finished("s1", "RUNNING").is_err());
    }

    #[test]
    fn maps_step_mode() {
        assert_eq!(
            mode_for_step(&Step {
                id: "s".into(),
                name: "S".into(),
                mode: "plan".into(),
                agent: None,
                model: None,
                effort: None,
                description: None,
                prompt: "x".into(),
                when: None,
                input_schema: None,
                output_schema: None,
            }),
            "PLAN_ONLY"
        );
    }

    #[tokio::test]
    async fn dry_run_completes_without_dispatch() {
        let test = crate::test_support::context().await;
        let context = RecipeContext::new(
            test.db.clone(),
            test.paths.clone(),
            crate::events::EventBus::new(32),
            std::sync::Arc::new(crate::secrets::SecretStore::new(&test.paths)),
        );
        let yaml = "id: dry\nname: Dry\ntrigger:\n  type: MANUAL\n  config: {}\nsteps:\n  - id: s1\n    name: Um\n    prompt: oi\n";
        store::save_yaml(&test.db, &test.workspace_id, "dry", yaml)
            .await
            .expect("save");
        // New recipes start disabled; this test exercises the run path, so it
        // performs the explicit activation a human would do after review.
        store::set_enabled(&test.db, &test.workspace_id, "dry", true)
            .await
            .expect("enable");
        let recipe = crate::recipes::parser::parse(yaml).expect("parse");
        let run = start(
            context.clone(),
            &test.workspace_id,
            recipe,
            serde_json::json!({}),
            true,
        )
        .await
        .expect("run");
        let mut status = run.status;
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            status = store::get_run(&test.db, &run.id).await.expect("get").status;
            if status == "COMPLETED" {
                break;
            }
        }
        assert_eq!(status, "COMPLETED");
    }

    #[test]
    fn replaces_attachment_safely_in_temp_dir() {
        let temp = tempfile::TempDir::new().expect("temp");
        let source = temp.path().join("new.md");
        let target = temp.path().join("out/final.md");
        std::fs::write(&source, "novo").expect("source");
        replace_attachment(
            &source.to_string_lossy(),
            &[target.to_string_lossy().to_string()],
        )
        .expect("replace");
        assert_eq!(std::fs::read_to_string(target).expect("target"), "novo");
    }
}
