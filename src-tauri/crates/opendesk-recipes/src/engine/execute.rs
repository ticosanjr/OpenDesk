use crate::{
    context::RecipeContext,
    error::{AppError, AppResult},
    hitl::{self, HitlOutcome, HitlResolution},
    parser::{self, FlowItem, Recipe},
    store,
};
use opendesk_storage::events::OpenDeskWsEvent;
use opendesk_storage::repos::tasks;

use chrono::Utc;
use serde_json::json;

use super::finalize::{finish, mark_routine, replace_attachment};
use super::steps::{execute_step, previous_step};

pub(super) async fn execute(
    context: &RecipeContext,
    recipe: &Recipe,
    run: &store::RecipeRun,
    trigger: &serde_json::Value,
    dispatcher: &dyn opendesk_engine::recipe_dispatch::RecipeStepDispatcher,
) -> AppResult<()> {
    if recipe.has_dag() {
        return crate::dag::execute(context, recipe, run, trigger, dispatcher).await;
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
                    .collect::<Result<Vec<_>, _>>()?;
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
