use crate::{
    context::RecipeContext,
    engine::{execute_step, finish, mark_routine},
    error::{AppError, AppResult},
    parser::{self, FlowItem, Recipe},
    store,
};
use chrono::Utc;
use futures_util::future::BoxFuture;
use opendesk_storage::events::OpenDeskWsEvent;
use opendesk_storage::repos::tasks;
use serde_json::{Value, json};

use super::groups::{run_loop, run_switch};
use super::hitl::handle_hitl;
use super::map::run_map;
use super::nodes::{account_step, ensure_node, finished, merge_node_output};
use super::parallel::run_parallel;
use super::paths::child_path;
use super::recipe_call::run_recipe_call;
use super::types::{Ctx, ScopeOutcome, ScopeState};

pub async fn execute(
    context: &RecipeContext,
    recipe: &Recipe,
    run: &store::RecipeRun,
    trigger: &Value,
    dispatcher: &dyn opendesk_engine::recipe_dispatch::RecipeStepDispatcher,
) -> AppResult<()> {
    let task_id = run
        .task_id
        .as_deref()
        .ok_or_else(|| AppError::BadRequest("recipe run sem task".into()))?;
    let run_vars = json!({
        "id": run.id,
        "output_dir": run.output_dir_path,
        "date": Utc::now().format("%Y-%m-%d").to_string(),
    });
    let routine_id = trigger
        .get("routine_id")
        .or_else(|| trigger.get("schedule_id"))
        .and_then(Value::as_str);
    let ctx = Ctx {
        context,
        recipe,
        run,
        task_id,
        trigger,
        run_vars: &run_vars,
        routine_id,
        dispatcher,
    };

    store::sync_run_nodes(&context.pool, &run.id, &recipe.flow).await?;
    let mut scope = ScopeState {
        outputs: serde_json::from_str(&run.step_outputs_json).unwrap_or_else(|_| json!({})),
        recipe_stack: vec![recipe.id.clone()],
        ..Default::default()
    };
    match run_scope(&ctx, &recipe.flow, None, &mut scope).await? {
        ScopeOutcome::Cancelled => return Ok(()),
        ScopeOutcome::Continue => {}
    }

    store::mark_pending_skipped(&context.pool, &run.id).await?;
    let _ = tasks::move_to(&context.pool, task_id, "DONE", None).await;
    let _ = tasks::set_phase_status(&context.pool, task_id, Some("done"), Some("DONE")).await;
    store::update_run(
        &context.pool,
        &run.id,
        "COMPLETED",
        None,
        Some(&scope.outputs),
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

pub(super) fn run_scope<'a>(
    ctx: &'a Ctx<'a>,
    items: &'a [FlowItem],
    parent: Option<&'a str>,
    scope: &'a mut ScopeState,
) -> BoxFuture<'a, AppResult<ScopeOutcome>> {
    Box::pin(async move {
        let mut index = 0usize;
        while index < items.len() {
            if store::is_cancel_requested(&ctx.context.pool, &ctx.run.id).await? {
                finish(
                    ctx.context,
                    ctx.run,
                    "REJECTED",
                    "cancelado pelo usuário",
                    ctx.routine_id,
                )
                .await?;
                return Ok(ScopeOutcome::Cancelled);
            }
            match &items[index] {
                FlowItem::Step(step) => {
                    let path = child_path(parent, &step.id);
                    let node =
                        ensure_node(ctx, &path, parent, &step.id, "step", None, None).await?;
                    if finished(&node) {
                        if node.status == "SKIPPED" {
                            scope.outputs[&step.id] = json!({ "skipped": true });
                        } else {
                            merge_node_output(scope, &node, &step.id);
                        }
                        index += 1;
                        continue;
                    }
                    let should_run = step
                        .when
                        .as_deref()
                        .map(|condition| {
                            parser::evaluate_when(
                                condition,
                                ctx.trigger,
                                ctx.run_vars,
                                &scope.outputs,
                            )
                        })
                        .transpose()?
                        .unwrap_or(true);
                    if !should_run {
                        store::mark_node_path(
                            &ctx.context.pool,
                            &ctx.run.id,
                            &path,
                            "SKIPPED",
                            None,
                            None,
                            None,
                        )
                        .await?;
                        scope.outputs[&step.id] = json!({
                            "skipped": true,
                            "reason": step.when,
                        });
                        index += 1;
                        continue;
                    }
                    store::mark_node_path(
                        &ctx.context.pool,
                        &ctx.run.id,
                        &path,
                        "RUNNING",
                        None,
                        None,
                        None,
                    )
                    .await?;
                    let feedback = scope.feedback.remove(&step.id);
                    match execute_step(
                        ctx.context,
                        ctx.dispatcher,
                        ctx.run,
                        ctx.task_id,
                        step,
                        ctx.trigger,
                        ctx.run_vars,
                        &mut scope.outputs,
                        &mut scope.session_id,
                        feedback,
                        ctx.routine_id,
                    )
                    .await
                    {
                        Ok(()) => {
                            store::mark_node_path(
                                &ctx.context.pool,
                                &ctx.run.id,
                                &path,
                                "DONE",
                                scope.outputs.get(&step.id),
                                scope.session_id.as_deref(),
                                None,
                            )
                            .await?;
                            account_step(ctx, scope, &step.id).await?;
                        }
                        Err(error) => {
                            if store::is_cancel_requested(&ctx.context.pool, &ctx.run.id).await? {
                                finish(
                                    ctx.context,
                                    ctx.run,
                                    "REJECTED",
                                    "cancelado pelo usuário",
                                    ctx.routine_id,
                                )
                                .await?;
                                return Ok(ScopeOutcome::Cancelled);
                            }
                            return Err(error);
                        }
                    }
                    index += 1;
                }
                FlowItem::Hitl(checkpoint) => {
                    let id = parser::checkpoint_id(checkpoint);
                    let path = child_path(parent, &id);
                    let node = ensure_node(ctx, &path, parent, &id, "hitl", None, None).await?;
                    if finished(&node) {
                        index += 1;
                        continue;
                    }
                    if !handle_hitl(ctx, checkpoint, &path, items, index, scope).await? {
                        return Ok(ScopeOutcome::Cancelled);
                    }
                    index += 1;
                }
                FlowItem::Sink(sink) => {
                    let (id, _) = parser::node_descriptor(&items[index], index);
                    let path = child_path(parent, &id);
                    let node = ensure_node(ctx, &path, parent, &id, "sink", None, None).await?;
                    if finished(&node) {
                        index += 1;
                        continue;
                    }
                    store::mark_node_path(
                        &ctx.context.pool,
                        &ctx.run.id,
                        &path,
                        "RUNNING",
                        None,
                        None,
                        None,
                    )
                    .await?;
                    store::update_run(
                        &ctx.context.pool,
                        &ctx.run.id,
                        "RUNNING_SINKS",
                        None,
                        Some(&scope.outputs),
                        None,
                        false,
                    )
                    .await?;
                    ctx.context
                        .sinks()
                        .execute(ctx.context, sink, ctx.run, ctx.trigger, &scope.outputs)
                        .await?;
                    store::mark_node_path(
                        &ctx.context.pool,
                        &ctx.run.id,
                        &path,
                        "DONE",
                        Some(&scope.outputs),
                        None,
                        None,
                    )
                    .await?;
                    index += 1;
                }
                FlowItem::Branch(branch) => {
                    let path = child_path(parent, &branch.id);
                    let node =
                        ensure_node(ctx, &path, parent, &branch.id, "branch", None, None).await?;
                    if finished(&node) {
                        index += 1;
                        continue;
                    }
                    store::mark_node_path(
                        &ctx.context.pool,
                        &ctx.run.id,
                        &path,
                        "RUNNING",
                        None,
                        None,
                        None,
                    )
                    .await?;
                    let selected = parser::evaluate_when(
                        &branch.when,
                        ctx.trigger,
                        ctx.run_vars,
                        &scope.outputs,
                    )?;
                    let branch_items = if selected {
                        &branch.then
                    } else {
                        &branch.otherwise
                    };
                    match run_scope(ctx, branch_items, Some(&path), scope).await? {
                        ScopeOutcome::Continue => {}
                        ScopeOutcome::Cancelled => return Ok(ScopeOutcome::Cancelled),
                    }
                    scope.outputs[&branch.id] = json!({
                        "then": selected,
                        "steps": branch_items.len(),
                    });
                    store::mark_node_path(
                        &ctx.context.pool,
                        &ctx.run.id,
                        &path,
                        "DONE",
                        Some(&json!({ "then": selected, "steps": branch_items.len() })),
                        None,
                        None,
                    )
                    .await?;
                    index += 1;
                }
                FlowItem::Parallel(parallel) => {
                    match run_parallel(ctx, parallel, parent, scope).await? {
                        ScopeOutcome::Continue => {}
                        ScopeOutcome::Cancelled => return Ok(ScopeOutcome::Cancelled),
                    }
                    index += 1;
                }
                FlowItem::Map(map) => {
                    match run_map(ctx, map, parent, scope).await? {
                        ScopeOutcome::Continue => {}
                        ScopeOutcome::Cancelled => return Ok(ScopeOutcome::Cancelled),
                    }
                    index += 1;
                }
                FlowItem::Recipe(call) => {
                    match run_recipe_call(ctx, call, parent, scope).await? {
                        ScopeOutcome::Continue => {}
                        ScopeOutcome::Cancelled => return Ok(ScopeOutcome::Cancelled),
                    }
                    index += 1;
                }
                FlowItem::Loop(loop_flow) => {
                    match run_loop(ctx, loop_flow, parent, scope).await? {
                        ScopeOutcome::Continue => {}
                        ScopeOutcome::Cancelled => return Ok(ScopeOutcome::Cancelled),
                    }
                    index += 1;
                }
                FlowItem::Switch(switch) => {
                    match run_switch(ctx, switch, parent, scope).await? {
                        ScopeOutcome::Continue => {}
                        ScopeOutcome::Cancelled => return Ok(ScopeOutcome::Cancelled),
                    }
                    index += 1;
                }
            }
        }
        Ok(ScopeOutcome::Continue)
    })
}
