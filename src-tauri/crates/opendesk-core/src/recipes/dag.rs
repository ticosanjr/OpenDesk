//! Path-addressed DAG executor for recipes with branch, parallel and map
//! groups. Flat recipes keep using the original sequential driver in
//! `engine.rs`.
//!
//! Every executable node has a stable path (`plan`, `lote/b0/revisar`,
//! `arquivos/0001/analisar`). Durability is keyed by that path, so a restart
//! resumes at the first node that is not DONE/SKIPPED even when the earlier
//! part of the flow contained concurrent groups.

use crate::{
    db::{self, tasks},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
    recipe_context::RecipeContext,
    recipes::{
        engine::{execute_step, finish, mark_routine, replace_attachment},
        hitl::{self, HitlOutcome, HitlResolution},
        parser::{self, FlowItem, HitlCheckpoint, Recipe, Step},
        store,
    },
};
use chrono::Utc;
use futures_util::{
    future::{BoxFuture, join_all},
    stream::{self, StreamExt},
};
use serde_json::{Value, json};
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Ctx<'a> {
    context: &'a RecipeContext,
    recipe: &'a Recipe,
    run: &'a store::RecipeRun,
    task_id: &'a str,
    trigger: &'a Value,
    run_vars: &'a Value,
    routine_id: Option<&'a str>,
    dispatcher: &'a dyn crate::recipe_dispatch::RecipeStepDispatcher,
}

#[derive(Default)]
struct ScopeState {
    outputs: Value,
    session_id: Option<String>,
    feedback: HashMap<String, String>,
    review_rounds: usize,
    recipe_stack: Vec<String>,
    spent_usd: f64,
    budget_usd: Option<f64>,
}

enum ScopeOutcome {
    Continue,
    Cancelled,
}

struct MapItemResult {
    order: usize,
    key: String,
    item: Value,
    outputs: Value,
    spent_usd: f64,
}

pub async fn execute(
    context: &RecipeContext,
    recipe: &Recipe,
    run: &store::RecipeRun,
    trigger: &Value,
    dispatcher: &dyn crate::recipe_dispatch::RecipeStepDispatcher,
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

fn run_scope<'a>(
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

async fn run_switch(
    ctx: &Ctx<'_>,
    switch: &parser::SwitchFlow,
    parent: Option<&str>,
    scope: &mut ScopeState,
) -> AppResult<ScopeOutcome> {
    let path = child_path(parent, &switch.id);
    let node = ensure_node(ctx, &path, parent, &switch.id, "switch", None, None).await?;
    if finished(&node) {
        return Ok(ScopeOutcome::Continue);
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
    let mut selected = None;
    for (index, case) in switch.cases.iter().enumerate() {
        if parser::evaluate_when(&case.when, ctx.trigger, ctx.run_vars, &scope.outputs)? {
            selected = Some(index);
            break;
        }
    }
    let items = selected
        .map(|index| switch.cases[index].flow.as_slice())
        .unwrap_or(switch.otherwise.as_slice());
    match run_scope(ctx, items, Some(&path), scope).await? {
        ScopeOutcome::Continue => {}
        ScopeOutcome::Cancelled => return Ok(ScopeOutcome::Cancelled),
    }
    let output = json!({ "case": selected, "steps": items.len() });
    scope.outputs[&switch.id] = output.clone();
    store::mark_node_path(
        &ctx.context.pool,
        &ctx.run.id,
        &path,
        "DONE",
        Some(&output),
        None,
        None,
    )
    .await?;
    Ok(ScopeOutcome::Continue)
}

async fn run_loop(
    ctx: &Ctx<'_>,
    loop_flow: &parser::LoopFlow,
    parent: Option<&str>,
    scope: &mut ScopeState,
) -> AppResult<ScopeOutcome> {
    let path = child_path(parent, &loop_flow.id);
    let node = ensure_node(ctx, &path, parent, &loop_flow.id, "loop", None, None).await?;
    if finished(&node) {
        return Ok(ScopeOutcome::Continue);
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
    let mut iterations = 0usize;
    for index in 0..loop_flow.max_iterations {
        let iteration_id = index.to_string();
        let iteration_path = child_path(Some(&path), &iteration_id);
        let marker = ensure_node(
            ctx,
            &iteration_path,
            Some(&path),
            &iteration_id,
            "iteration",
            None,
            None,
        )
        .await?;
        if !finished(&marker) {
            store::mark_node_path(
                &ctx.context.pool,
                &ctx.run.id,
                &iteration_path,
                "RUNNING",
                None,
                None,
                None,
            )
            .await?;
            match run_scope(ctx, &loop_flow.flow, Some(&iteration_path), scope).await? {
                ScopeOutcome::Continue => {}
                ScopeOutcome::Cancelled => return Ok(ScopeOutcome::Cancelled),
            }
            store::mark_node_path(
                &ctx.context.pool,
                &ctx.run.id,
                &iteration_path,
                "DONE",
                None,
                None,
                None,
            )
            .await?;
        }
        iterations = index + 1;
        let output = json!({ "iterations": iterations });
        scope.outputs[&loop_flow.id] = output;
        if let Some(until) = loop_flow.until.as_deref()
            && parser::evaluate_when(until, ctx.trigger, ctx.run_vars, &scope.outputs)?
        {
            break;
        }
    }
    let output = json!({ "iterations": iterations });
    scope.outputs[&loop_flow.id] = output.clone();
    store::mark_node_path(
        &ctx.context.pool,
        &ctx.run.id,
        &path,
        "DONE",
        Some(&output),
        None,
        None,
    )
    .await?;
    Ok(ScopeOutcome::Continue)
}

async fn run_parallel(
    ctx: &Ctx<'_>,
    parallel: &parser::ParallelFlow,
    parent: Option<&str>,
    scope: &mut ScopeState,
) -> AppResult<ScopeOutcome> {
    let path = child_path(parent, &parallel.id);
    let node = ensure_node(
        ctx,
        &path,
        parent,
        &parallel.id,
        "parallel",
        None,
        Some(&parallel.join),
    )
    .await?;
    if finished(&node) {
        restore_finished(ctx, &path, &node, scope).await?;
        return Ok(ScopeOutcome::Continue);
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
    liveness(ctx, &parallel.id, &scope.outputs).await?;

    let mut futures = Vec::with_capacity(parallel.branches.len());
    for (branch_index, branch) in parallel.branches.iter().enumerate() {
        let branch_path = child_path(Some(&path), &format!("b{branch_index}"));
        let mut local = ScopeState {
            outputs: scope.outputs.clone(),
            recipe_stack: scope.recipe_stack.clone(),
            budget_usd: scope.budget_usd,
            ..Default::default()
        };
        futures.push(async move {
            let result = run_scope(ctx, branch, Some(&branch_path), &mut local).await;
            (result, local)
        });
    }
    let results = join_all(futures).await;

    let mut merged = scope.outputs.clone();
    let mut cancelled = false;
    let mut error = None;
    let mut succeeded = 0usize;
    for (result, local) in &results {
        match result {
            Ok(ScopeOutcome::Continue) => {
                succeeded += 1;
                merge_outputs(&mut merged, &local.outputs);
            }
            Ok(ScopeOutcome::Cancelled) => cancelled = true,
            Err(failure) => {
                if error.is_none() {
                    error = Some(format!("{failure:#}"));
                }
            }
        }
    }
    if cancelled {
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

    let completed =
        succeeded > 0 && (parallel.join == "all" && error.is_none() || parallel.join == "any");
    if !completed {
        let message = error.unwrap_or_else(|| "nenhum branch concluiu".into());
        store::mark_node_path(
            &ctx.context.pool,
            &ctx.run.id,
            &path,
            "FAILED",
            None,
            None,
            Some(&message),
        )
        .await?;
        return Err(AppError::BadRequest(format!(
            "parallel {} falhou: {message}",
            parallel.id
        )));
    }
    let spent_usd: f64 = results.iter().map(|(_, local)| local.spent_usd).sum();
    scope.spent_usd += spent_usd;
    enforce_budget(scope)?;
    scope.outputs = merged;
    scope.outputs[&parallel.id] = json!({
        "spent_usd": spent_usd,
        "join": parallel.join,
        "branches": parallel.branches.len(),
        "succeeded": succeeded,
    });
    store::mark_node_path(
        &ctx.context.pool,
        &ctx.run.id,
        &path,
        "DONE",
        Some(&json!({
            "join": parallel.join,
            "branches": parallel.branches.len(),
            "succeeded": succeeded,
            "spent_usd": spent_usd,
        })),
        None,
        None,
    )
    .await?;
    Ok(ScopeOutcome::Continue)
}

async fn run_map(
    ctx: &Ctx<'_>,
    map: &parser::MapFlow,
    parent: Option<&str>,
    scope: &mut ScopeState,
) -> AppResult<ScopeOutcome> {
    let path = child_path(parent, &map.id);
    let node = ensure_node(ctx, &path, parent, &map.id, "map", None, None).await?;
    if finished(&node) {
        if let Some(output) = node
            .output_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        {
            scope.outputs[&map.id] = output;
        }
        return Ok(ScopeOutcome::Continue);
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
    liveness(ctx, &map.id, &scope.outputs).await?;

    let rendered = parser::render(&map.items, ctx.trigger, ctx.run_vars, &scope.outputs)?;
    let items: Vec<Value> = if rendered.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&rendered).map_err(|error| {
            AppError::BadRequest(format!("items do map {} não é JSON: {error}", map.id))
        })?
    };
    let base_outputs = scope.outputs.clone();
    let recipe_stack = scope.recipe_stack.clone();
    let budget_usd = scope.budget_usd;
    let max_concurrency = map.max_concurrency.clamp(1, 8);
    let group_path = path.clone();
    let jobs = items.into_iter().enumerate().map(move |(index, item)| {
        let key = item_key(&item, index);
        let item_scope_path = format!("{group_path}/{key}");
        let parent_path = group_path.clone();
        let mut local = ScopeState {
            outputs: base_outputs.clone(),
            recipe_stack: recipe_stack.clone(),
            budget_usd,
            ..Default::default()
        };
        local.outputs[&map.item_var] = item.clone();
        local.outputs[&map.id] = json!({
            "item": item,
            "index": index,
            "key": key,
        });
        let item_for_result = item.clone();
        let index_for_result = index;
        async move {
            let item_node = ensure_node(
                ctx,
                &item_scope_path,
                Some(&parent_path),
                &key,
                "map_item",
                Some(&key),
                None,
            )
            .await?;
            if finished(&item_node) {
                let stored = item_node
                    .output_json
                    .as_deref()
                    .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
                    .unwrap_or_else(|| json!({}));
                return Ok::<_, AppError>((
                    ScopeOutcome::Continue,
                    MapItemResult {
                        order: index_for_result,
                        key,
                        item: item_for_result,
                        outputs: stored.get("outputs").cloned().unwrap_or(stored.clone()),
                        spent_usd: stored
                            .get("spent_usd")
                            .and_then(Value::as_f64)
                            .unwrap_or(0.0),
                    },
                ));
            }
            store::mark_node_path(
                &ctx.context.pool,
                &ctx.run.id,
                &item_scope_path,
                "RUNNING",
                None,
                None,
                None,
            )
            .await?;
            let outcome = run_scope(ctx, &map.flow, Some(&item_scope_path), &mut local).await?;
            store::mark_node_path(
                &ctx.context.pool,
                &ctx.run.id,
                &item_scope_path,
                "DONE",
                Some(&json!({
                    "outputs": local.outputs,
                    "spent_usd": local.spent_usd,
                })),
                local.session_id.as_deref(),
                None,
            )
            .await?;
            Ok::<_, AppError>((
                outcome,
                MapItemResult {
                    order: index_for_result,
                    key,
                    item: item_for_result,
                    outputs: local.outputs,
                    spent_usd: local.spent_usd,
                },
            ))
        }
    });

    let mut results = Vec::new();
    let mut stream = stream::iter(jobs).buffer_unordered(max_concurrency);
    while let Some(result) = stream.next().await {
        let (outcome, item) = result?;
        if matches!(outcome, ScopeOutcome::Cancelled) {
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
        results.push(item);
    }
    results.sort_by_key(|result| result.order);

    let spent_usd: f64 = results.iter().map(|result| result.spent_usd).sum();
    scope.spent_usd += spent_usd;
    enforce_budget(scope)?;
    let summary = json!({
        "count": results.len(),
        "spent_usd": spent_usd,
        "items": results.iter().map(|result| json!({
            "key": result.key,
            "item": result.item,
            "outputs": result.outputs,
        })).collect::<Vec<_>>(),
    });
    scope.outputs[&map.id] = summary.clone();
    store::mark_node_path(
        &ctx.context.pool,
        &ctx.run.id,
        &path,
        "DONE",
        Some(&summary),
        None,
        None,
    )
    .await?;
    Ok(ScopeOutcome::Continue)
}

async fn run_recipe_call(
    ctx: &Ctx<'_>,
    call: &parser::RecipeCall,
    parent: Option<&str>,
    scope: &mut ScopeState,
) -> AppResult<ScopeOutcome> {
    let path = child_path(parent, &call.id);
    let node = ensure_node(ctx, &path, parent, &call.id, "recipe", None, None).await?;
    if finished(&node) {
        restore_finished(ctx, &path, &node, scope).await?;
        return Ok(ScopeOutcome::Continue);
    }
    let should_run = call
        .when
        .as_deref()
        .map(|condition| {
            parser::evaluate_when(condition, ctx.trigger, ctx.run_vars, &scope.outputs)
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
        return Ok(ScopeOutcome::Continue);
    }
    if scope.recipe_stack.len() >= 8 {
        return Err(AppError::BadRequest(
            "profundidade máxima de sub-receitas excedida".into(),
        ));
    }
    let yaml = store::yaml_for(&ctx.context.pool, &ctx.run.workspace_id, &call.recipe).await?;
    let sub = parser::parse(&yaml)?;
    if scope.recipe_stack.iter().any(|id| id == &sub.id) {
        return Err(AppError::BadRequest(format!(
            "ciclo de sub-receitas detectado: {} -> {}",
            scope.recipe_stack.join(" -> "),
            sub.id
        )));
    }
    let inputs = render_inputs(&call.inputs, ctx.trigger, ctx.run_vars, &scope.outputs)?;
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
    liveness(ctx, &call.id, &scope.outputs).await?;

    let sub_ctx = Ctx {
        recipe: &sub,
        trigger: &inputs,
        ..*ctx
    };
    let mut local = ScopeState {
        outputs: scope.outputs.clone(),
        recipe_stack: {
            let mut stack = scope.recipe_stack.clone();
            stack.push(sub.id.clone());
            stack
        },
        budget_usd: call.budget_usd,
        ..Default::default()
    };
    match run_scope(&sub_ctx, &sub.flow, Some(&path), &mut local).await? {
        ScopeOutcome::Continue => {}
        ScopeOutcome::Cancelled => return Ok(ScopeOutcome::Cancelled),
    }
    scope.spent_usd += local.spent_usd;
    enforce_budget(scope)?;
    let output = json!({
        "recipe": sub.id,
        "outputs": local.outputs,
        "spent_usd": local.spent_usd,
    });
    scope.outputs[&call.id] = output.clone();
    store::mark_node_path(
        &ctx.context.pool,
        &ctx.run.id,
        &path,
        "DONE",
        Some(&output),
        local.session_id.as_deref(),
        None,
    )
    .await?;
    Ok(ScopeOutcome::Continue)
}

fn render_inputs(
    inputs: &Value,
    trigger: &Value,
    run_vars: &Value,
    outputs: &Value,
) -> AppResult<Value> {
    match inputs {
        Value::String(template) => Ok(Value::String(parser::render(
            template, trigger, run_vars, outputs,
        )?)),
        Value::Array(items) => Ok(Value::Array(
            items
                .iter()
                .map(|item| render_inputs(item, trigger, run_vars, outputs))
                .collect::<AppResult<Vec<_>>>()?,
        )),
        Value::Object(object) => {
            let mut rendered = serde_json::Map::new();
            for (key, value) in object {
                rendered.insert(
                    key.clone(),
                    render_inputs(value, trigger, run_vars, outputs)?,
                );
            }
            Ok(Value::Object(rendered))
        }
        Value::Null => Ok(json!({})),
        other => Ok(other.clone()),
    }
}

async fn handle_hitl(
    ctx: &Ctx<'_>,
    checkpoint: &HitlCheckpoint,
    path: &str,
    items: &[FlowItem],
    index: usize,
    scope: &mut ScopeState,
) -> AppResult<bool> {
    let mut checkpoint = checkpoint.clone();
    checkpoint.id = parser::checkpoint_id(&checkpoint);
    let current_step = previous_step(items, index).map(|(_, step)| step.id.clone());
    store::update_run(
        &ctx.context.pool,
        &ctx.run.id,
        "WAITING_HITL",
        current_step.as_deref(),
        Some(&scope.outputs),
        None,
        false,
    )
    .await?;
    store::mark_node_path(
        &ctx.context.pool,
        &ctx.run.id,
        path,
        "WAITING_HITL",
        None,
        None,
        None,
    )
    .await?;
    let _ = tasks::move_to(&ctx.context.pool, ctx.task_id, "IN_REVIEW", None).await;
    let _ = tasks::set_phase_status(
        &ctx.context.pool,
        ctx.task_id,
        Some("waiting_hitl"),
        Some("IN_REVIEW"),
    )
    .await;
    let message = parser::render(
        &checkpoint.prompt_template,
        ctx.trigger,
        ctx.run_vars,
        &scope.outputs,
    )?;
    let attachments = checkpoint
        .attach_files
        .iter()
        .map(|path| parser::render(path, ctx.trigger, ctx.run_vars, &scope.outputs))
        .collect::<Result<Vec<_>, _>>()?;
    let next_step = next_item_name(items, index + 1);
    let token =
        match store::pending_hitl_for_checkpoint(&ctx.context.pool, &ctx.run.id, &checkpoint.id)
            .await?
        {
            Some(existing) => existing.token,
            None => {
                hitl::request(
                    ctx.context,
                    ctx.run,
                    &checkpoint,
                    &ctx.recipe.name,
                    &message,
                    next_step,
                    &attachments,
                )
                .await?
            }
        };
    ctx.context.events.publish(OpenDeskWsEvent::HitlRequested {
        token: token.clone(),
        run_id: ctx.run.id.clone(),
        workspace_id: ctx.run.workspace_id.clone(),
        checkpoint_id: checkpoint.id.clone(),
        interaction_type: checkpoint.interaction_type.clone(),
    });
    let outcome = hitl::wait(ctx.context, &token, checkpoint.timeout_minutes).await?;
    match outcome {
        HitlOutcome::Resolved(HitlResolution::Approved) => {
            ctx.context.events.publish(OpenDeskWsEvent::HitlResolved {
                token,
                status: "APPROVED".into(),
            });
            store::mark_node_path(
                &ctx.context.pool,
                &ctx.run.id,
                path,
                "DONE",
                None,
                None,
                None,
            )
            .await?;
            Ok(true)
        }
        HitlOutcome::Resolved(HitlResolution::Rejected { reason }) => {
            finish(
                ctx.context,
                ctx.run,
                "REJECTED",
                reason.as_deref().unwrap_or("rejeitado pelo usuário"),
                ctx.routine_id,
            )
            .await?;
            Ok(false)
        }
        HitlOutcome::Resolved(HitlResolution::FeedbackProvided {
            feedback_text,
            attached_files,
        }) => {
            if let Some(file) = attached_files.first() {
                replace_attachment(file, &attachments)?;
                store::mark_node_path(
                    &ctx.context.pool,
                    &ctx.run.id,
                    path,
                    "DONE",
                    None,
                    None,
                    None,
                )
                .await?;
                return Ok(true);
            }
            replay_previous(ctx, items, index, path, &feedback_text, scope).await
        }
        HitlOutcome::Resolved(HitlResolution::ChoiceSelected { option_value, .. }) => {
            let feedback = format!("Opção escolhida: {option_value}");
            replay_previous(ctx, items, index, path, &feedback, scope).await
        }
        HitlOutcome::Resolved(HitlResolution::ArtifactEdited { path: edited }) => {
            replace_attachment(&edited, &attachments)?;
            store::mark_node_path(
                &ctx.context.pool,
                &ctx.run.id,
                path,
                "DONE",
                None,
                None,
                None,
            )
            .await?;
            Ok(true)
        }
        HitlOutcome::Expired => {
            if checkpoint.on_timeout == "AUTO_APPROVE" {
                store::mark_node_path(
                    &ctx.context.pool,
                    &ctx.run.id,
                    path,
                    "DONE",
                    None,
                    None,
                    None,
                )
                .await?;
                Ok(true)
            } else {
                finish(
                    ctx.context,
                    ctx.run,
                    "TIMED_OUT",
                    "timeout do checkpoint HITL",
                    ctx.routine_id,
                )
                .await?;
                Ok(false)
            }
        }
        HitlOutcome::Cancelled => {
            finish(
                ctx.context,
                ctx.run,
                "REJECTED",
                "cancelado pelo usuário",
                ctx.routine_id,
            )
            .await?;
            Ok(false)
        }
    }
}

/// Records feedback for the previous step and rewinds the scope so it runs
/// again. The caller only advances after the HITL is approved.
async fn replay_previous(
    ctx: &Ctx<'_>,
    items: &[FlowItem],
    index: usize,
    hitl_path: &str,
    feedback: &str,
    scope: &mut ScopeState,
) -> AppResult<bool> {
    scope.review_rounds += 1;
    if scope.review_rounds > 5 {
        finish(
            ctx.context,
            ctx.run,
            "REJECTED",
            "limite de revisões atingido",
            ctx.routine_id,
        )
        .await?;
        return Ok(false);
    }
    let Some((_previous_index, previous_step)) = previous_step(items, index) else {
        finish(
            ctx.context,
            ctx.run,
            "REJECTED",
            "checkpoint sem step anterior para refazer",
            ctx.routine_id,
        )
        .await?;
        return Ok(false);
    };
    let previous_path = child_path(parent_path(hitl_path), &previous_step.id);
    scope
        .feedback
        .insert(previous_step.id.clone(), feedback.to_string());
    store::reset_node_path(&ctx.context.pool, &ctx.run.id, &previous_path).await?;
    store::reset_node_path(&ctx.context.pool, &ctx.run.id, hitl_path).await?;
    Ok(true)
}

async fn restore_finished(
    ctx: &Ctx<'_>,
    path: &str,
    node: &store::RecipeRunNode,
    scope: &mut ScopeState,
) -> AppResult<()> {
    if let Some(output) = parse_output(node) {
        if let Some(spent) = output.get("spent_usd").and_then(Value::as_f64) {
            scope.spent_usd += spent.max(0.0);
        }
        scope.outputs[&node.node_id] = output;
    }
    if node.kind == "map" || node.kind == "recipe" {
        return Ok(());
    }
    let children = store::list_nodes_under(&ctx.context.pool, &ctx.run.id, path).await?;
    // Descendants of a nested map are represented by the map group output;
    // restoring each item step would overwrite the same step id.
    let map_prefixes: Vec<String> = children
        .iter()
        .filter(|child| child.kind == "map")
        .filter_map(|child| child.node_path.clone())
        .collect();
    for child in children {
        let Some(child_path) = child.node_path.as_deref() else {
            continue;
        };
        if map_prefixes
            .iter()
            .any(|prefix| child_path != prefix && child_path.starts_with(&format!("{prefix}/")))
        {
            continue;
        }
        if matches!(child.kind.as_str(), "map_item") {
            continue;
        }
        if let Some(output) = parse_output(&child) {
            scope.outputs[&child.node_id] = output;
        }
    }
    Ok(())
}

async fn account_step(ctx: &Ctx<'_>, scope: &mut ScopeState, step_id: &str) -> AppResult<()> {
    let run_id = scope
        .outputs
        .get(step_id)
        .and_then(|value| value.get("run_id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(run_id) = run_id
        && let Ok(run) = db::runs::get(&ctx.context.pool, &run_id).await
    {
        scope.spent_usd += run.estimated_cost_usd.max(0.0);
    }
    enforce_budget(scope)
}

fn enforce_budget(scope: &ScopeState) -> AppResult<()> {
    if let Some(limit) = scope.budget_usd
        && scope.spent_usd > limit + 1e-9
    {
        return Err(AppError::BadRequest(format!(
            "orçamento da sub-receita excedido: ${:.4} > ${:.4}",
            scope.spent_usd, limit
        )));
    }
    Ok(())
}

fn parse_output(node: &store::RecipeRunNode) -> Option<Value> {
    node.output_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
}

fn merge_node_output(scope: &mut ScopeState, node: &store::RecipeRunNode, step_id: &str) {
    if let Some(output) = parse_output(node) {
        scope.outputs[step_id] = output;
    }
}

async fn ensure_node(
    ctx: &Ctx<'_>,
    path: &str,
    parent: Option<&str>,
    node_id: &str,
    kind: &str,
    item_key: Option<&str>,
    join_policy: Option<&str>,
) -> AppResult<store::RecipeRunNode> {
    store::ensure_dag_node(
        &ctx.context.pool,
        &ctx.run.id,
        path,
        parent,
        node_id,
        kind,
        item_key,
        join_policy,
    )
    .await?;
    store::node_by_path(&ctx.context.pool, &ctx.run.id, path)
        .await?
        .ok_or_else(|| AppError::Internal(format!("nó de receita ausente: {path}")))
}

fn finished(node: &store::RecipeRunNode) -> bool {
    matches!(node.status.as_str(), "DONE" | "SKIPPED")
}

async fn liveness(ctx: &Ctx<'_>, node_id: &str, outputs: &Value) -> AppResult<()> {
    store::update_run(
        &ctx.context.pool,
        &ctx.run.id,
        "RUNNING_STEP",
        Some(node_id),
        Some(outputs),
        None,
        false,
    )
    .await?;
    Ok(())
}

fn previous_step(items: &[FlowItem], index: usize) -> Option<(usize, &Step)> {
    items[..index]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(position, item)| match item {
            FlowItem::Step(step) => Some((position, step)),
            _ => None,
        })
}

fn next_item_name(items: &[FlowItem], index: usize) -> Option<&str> {
    items.get(index).and_then(|item| match item {
        FlowItem::Step(step) => Some(step.name.as_str()),
        FlowItem::Hitl(checkpoint) => Some(checkpoint.id.as_str()),
        FlowItem::Sink(sink) => sink.name.as_deref(),
        FlowItem::Branch(branch) => Some(branch.name.as_deref().unwrap_or(&branch.id)),
        FlowItem::Parallel(parallel) => Some(parallel.name.as_deref().unwrap_or(&parallel.id)),
        FlowItem::Map(map) => Some(map.name.as_deref().unwrap_or(&map.id)),
        FlowItem::Recipe(call) => Some(call.name.as_deref().unwrap_or(&call.id)),
        FlowItem::Loop(loop_flow) => Some(loop_flow.name.as_deref().unwrap_or(&loop_flow.id)),
        FlowItem::Switch(switch) => Some(switch.name.as_deref().unwrap_or(&switch.id)),
    })
}

fn child_path(parent: Option<&str>, id: &str) -> String {
    match parent {
        Some(parent) if !parent.is_empty() => format!("{parent}/{id}"),
        _ => id.to_string(),
    }
}

fn parent_path(path: &str) -> Option<&str> {
    path.rsplit_once('/').map(|(parent, _)| parent)
}

fn item_key(item: &Value, index: usize) -> String {
    if let Some(path) = item.as_str() {
        return format!("{index:04}-{}", file_stem(path));
    }
    if let Some(key) = item.get("key").and_then(Value::as_str)
        && !key.trim().is_empty()
    {
        let sanitized: String = key
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                    character
                } else {
                    '-'
                }
            })
            .collect();
        return format!("{index:04}-{sanitized}");
    }
    if let Some(name) = item.get("path").and_then(Value::as_str) {
        return format!("{index:04}-{}", file_stem(name));
    }
    format!("{index:04}")
}

fn file_stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "item".into())
}

fn merge_outputs(target: &mut Value, source: &Value) {
    if let (Some(target), Some(source)) = (target.as_object_mut(), source.as_object()) {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        events::EventBus, recipe_context::RecipeContext, recipes::parser, secrets::SecretStore,
    };
    use std::sync::Arc;

    fn recipe_context(ctx: &crate::test_support::TestContext) -> RecipeContext {
        RecipeContext::new(
            ctx.db.clone(),
            ctx.paths.clone(),
            EventBus::new(32),
            Arc::new(SecretStore::new(&ctx.paths)),
        )
    }

    const YAML: &str = r#"
id: dag-exec
name: DAG exec
flow:
  - type: branch
    id: decidir
    when: "true"
    then:
      - type: step
        id: t1
        name: Then
        prompt: a
        when: "false"
    else:
      - type: step
        id: e1
        name: Else
        prompt: b
        when: "false"
  - type: parallel
    id: lote
    join: all
    branches:
      - - type: step
          id: p1
          name: P1
          prompt: a
          when: "false"
      - - type: step
          id: p2
          name: P2
          prompt: b
          when: "false"
  - type: map
    id: arquivos
    items: "[]"
    flow:
      - type: step
        id: analisar
        name: Analisar
        prompt: c
"#;

    async fn fixture_with(
        yaml: &str,
        id: &str,
    ) -> (
        crate::test_support::TestContext,
        parser::Recipe,
        store::RecipeRun,
    ) {
        let ctx = crate::test_support::context().await;
        store::save_yaml(&ctx.db, &ctx.workspace_id, id, yaml)
            .await
            .expect("save");
        let recipe = parser::parse(yaml).expect("recipe");
        let task = tasks::create(
            &ctx.db,
            &ctx.workspace_id,
            tasks::CreateTaskInput {
                title: "dag".into(),
                description: "dag".into(),
                execution_mode: Some("BUILD_DIRECT".into()),
                priority: Some(2),
                assigned_agent: None,
                preferred_model: None,
                preferred_harness: None,
                effort_level: None,
                care_level: Some("standard".into()),
                notify_email: None,
                export_to_gdocs: false,
                tags: vec![],
                origin: Some("RECIPE".into()),
                parent_task_id: None,
                dedupe: None,
            },
        )
        .await
        .expect("task");
        let run = store::create_run(
            &ctx.db,
            &recipe,
            &ctx.workspace_id,
            Some(&task.id),
            &json!({}),
            "/tmp/dag-out",
            false,
        )
        .await
        .expect("run");
        (ctx, recipe, run)
    }

    async fn fixture() -> (
        crate::test_support::TestContext,
        parser::Recipe,
        store::RecipeRun,
    ) {
        fixture_with(YAML, "dag-exec").await
    }

    #[tokio::test]
    async fn executes_switch_and_bounded_loop_with_durable_paths() {
        let yaml = r#"
id: loop-switch
name: Loop
flow:
  - type: switch
    id: escolher
    cases:
      - when: "true"
        flow:
          - type: step
            id: s1
            name: S1
            prompt: a
            when: "false"
      - when: "false"
        flow:
          - type: step
            id: s2
            name: S2
            prompt: b
            when: "false"
    default:
      - type: step
        id: s3
        name: S3
        prompt: c
        when: "false"
  - type: loop
    id: repetir
    max_iterations: 2
    flow:
      - type: step
        id: l1
        name: L1
        prompt: d
        when: "false"
"#;
        let (ctx, recipe, run) = fixture_with(yaml, "loop-switch").await;
        execute(
            &recipe_context(&ctx),
            &recipe,
            &run,
            &json!({}),
            &crate::recipe_dispatch::NoopRecipeDispatcher,
        )
        .await
        .expect("execute");
        let completed = store::get_run(&ctx.db, &run.id).await.expect("run");
        assert_eq!(completed.status, "COMPLETED");
        for path in [
            "escolher",
            "escolher/s1",
            "repetir",
            "repetir/0",
            "repetir/0/l1",
        ] {
            assert!(
                store::node_by_path(&ctx.db, &run.id, path)
                    .await
                    .expect("node")
                    .is_some(),
                "missing {path}"
            );
        }
    }

    #[tokio::test]
    async fn executes_branch_parallel_and_empty_map_without_dispatching() {
        let (ctx, recipe, run) = fixture().await;
        execute(
            &recipe_context(&ctx),
            &recipe,
            &run,
            &json!({}),
            &crate::recipe_dispatch::NoopRecipeDispatcher,
        )
        .await
        .expect("execute");

        let completed = store::get_run(&ctx.db, &run.id).await.expect("run");
        assert_eq!(completed.status, "COMPLETED");
        let outputs: Value = serde_json::from_str(&completed.step_outputs_json).expect("outputs");
        assert_eq!(outputs["decidir"]["then"], true);
        assert_eq!(outputs["arquivos"]["count"], 0);

        let nodes = store::list_run_nodes(&ctx.db, &run.id)
            .await
            .expect("nodes");
        for expected in [
            ("decidir", "DONE"),
            ("decidir/t1", "SKIPPED"),
            ("decidir/e1", "SKIPPED"),
            ("lote", "DONE"),
            ("lote/b0/p1", "SKIPPED"),
            ("lote/b1/p2", "SKIPPED"),
            ("arquivos", "DONE"),
        ] {
            let node = nodes
                .iter()
                .find(|node| node.node_path.as_deref() == Some(expected.0))
                .unwrap_or_else(|| panic!("missing {}", expected.0));
            assert_eq!(node.status, expected.1, "node {}", expected.0);
        }
    }

    #[tokio::test]
    async fn expands_map_items_into_durable_paths() {
        let yaml = r#"
id: dag-map
name: DAG map
flow:
  - type: map
    id: mapa
    items: '[{"path":"a.txt"},{"path":"b.txt"}]'
    max_concurrency: 2
    flow:
      - type: step
        id: analisar
        name: Analisar
        prompt: "Analise {{item.path}}"
        when: "false"
"#;
        let (ctx, recipe, run) = fixture_with(yaml, "dag-map").await;
        execute(
            &recipe_context(&ctx),
            &recipe,
            &run,
            &json!({}),
            &crate::recipe_dispatch::NoopRecipeDispatcher,
        )
        .await
        .expect("execute");

        let completed = store::get_run(&ctx.db, &run.id).await.expect("run");
        let outputs: Value = serde_json::from_str(&completed.step_outputs_json).expect("outputs");
        assert_eq!(outputs["mapa"]["count"], 2);
        assert_eq!(outputs["mapa"]["items"][0]["key"], "0000-a");
        assert_eq!(outputs["mapa"]["items"][1]["key"], "0001-b");

        let nodes = store::list_run_nodes(&ctx.db, &run.id)
            .await
            .expect("nodes");
        for path in [
            "mapa/0000-a",
            "mapa/0000-a/analisar",
            "mapa/0001-b",
            "mapa/0001-b/analisar",
        ] {
            assert!(
                nodes
                    .iter()
                    .any(|node| node.node_path.as_deref() == Some(path)),
                "missing node {path}"
            );
        }
    }

    #[tokio::test]
    async fn calls_a_sub_recipe_with_isolated_outputs() {
        let child = r#"
id: dag-child
name: Child
flow:
  - type: step
    id: child-step
    name: Child step
    prompt: "{{trigger.topic}}"
    when: "false"
"#;
        let parent = r#"
id: dag-parent
name: Parent
flow:
  - type: recipe
    id: chamar
    recipe: dag-child
    with:
      topic: "assunto"
  - type: step
    id: after
    name: After
    prompt: x
    when: "false"
"#;
        let ctx = crate::test_support::context().await;
        store::save_yaml(&ctx.db, &ctx.workspace_id, "dag-child", child)
            .await
            .expect("child");
        let (ctx, recipe, run) = {
            let saved = store::save_yaml(&ctx.db, &ctx.workspace_id, "dag-parent", parent)
                .await
                .expect("parent");
            let recipe = parser::parse(parent).expect("recipe");
            let task = tasks::create(
                &ctx.db,
                &ctx.workspace_id,
                tasks::CreateTaskInput {
                    title: "parent".into(),
                    description: "parent".into(),
                    execution_mode: Some("BUILD_DIRECT".into()),
                    priority: Some(2),
                    assigned_agent: None,
                    preferred_model: None,
                    preferred_harness: None,
                    effort_level: None,
                    care_level: Some("standard".into()),
                    notify_email: None,
                    export_to_gdocs: false,
                    tags: vec![],
                    origin: Some("RECIPE".into()),
                    parent_task_id: None,
                    dedupe: None,
                },
            )
            .await
            .expect("task");
            let run = store::create_run(
                &ctx.db,
                &recipe,
                &ctx.workspace_id,
                Some(&task.id),
                &json!({}),
                "/tmp/dag-parent",
                false,
            )
            .await
            .expect("run");
            let _ = saved;
            (ctx, recipe, run)
        };
        execute(
            &recipe_context(&ctx),
            &recipe,
            &run,
            &json!({}),
            &crate::recipe_dispatch::NoopRecipeDispatcher,
        )
        .await
        .expect("execute");

        let completed = store::get_run(&ctx.db, &run.id).await.expect("run");
        let outputs: Value = serde_json::from_str(&completed.step_outputs_json).expect("outputs");
        assert_eq!(outputs["chamar"]["recipe"], "dag-child");
        assert_eq!(outputs["chamar"]["outputs"]["child-step"]["skipped"], true);
        let nodes = store::list_run_nodes(&ctx.db, &run.id)
            .await
            .expect("nodes");
        assert!(
            nodes.iter().any(
                |node| node.node_path.as_deref() == Some("chamar/child-step")
                    && node.status == "SKIPPED"
            ),
            "{nodes:?}"
        );
    }

    #[tokio::test]
    async fn rejects_sub_recipe_cycles() {
        let a = r#"
id: cycle-a
name: A
flow:
  - type: recipe
    id: call-b
    recipe: cycle-b
  - type: step
    id: a-step
    name: A step
    prompt: x
    when: "false"
"#;
        let b = r#"
id: cycle-b
name: B
flow:
  - type: recipe
    id: call-a
    recipe: cycle-a
  - type: step
    id: b-step
    name: B step
    prompt: x
    when: "false"
"#;
        let ctx = crate::test_support::context().await;
        store::save_yaml(&ctx.db, &ctx.workspace_id, "cycle-b", b)
            .await
            .expect("b");
        let recipe = parser::parse(a).expect("a");
        store::save_yaml(&ctx.db, &ctx.workspace_id, "cycle-a", a)
            .await
            .expect("a index");
        let task = tasks::create(
            &ctx.db,
            &ctx.workspace_id,
            tasks::CreateTaskInput {
                title: "cycle".into(),
                description: "cycle".into(),
                execution_mode: Some("BUILD_DIRECT".into()),
                priority: Some(2),
                assigned_agent: None,
                preferred_model: None,
                preferred_harness: None,
                effort_level: None,
                care_level: Some("standard".into()),
                notify_email: None,
                export_to_gdocs: false,
                tags: vec![],
                origin: Some("RECIPE".into()),
                parent_task_id: None,
                dedupe: None,
            },
        )
        .await
        .expect("task");
        let run = store::create_run(
            &ctx.db,
            &recipe,
            &ctx.workspace_id,
            Some(&task.id),
            &json!({}),
            "/tmp/cycle",
            false,
        )
        .await
        .expect("run");
        let error = execute(
            &recipe_context(&ctx),
            &recipe,
            &run,
            &json!({}),
            &crate::recipe_dispatch::NoopRecipeDispatcher,
        )
        .await
        .expect_err("cycle");
        assert!(format!("{error:#}").contains("ciclo"), "{error:#}");
    }

    #[tokio::test]
    async fn resume_skips_a_completed_step_and_restores_its_output() {
        let (ctx, recipe, run) = fixture().await;
        store::sync_run_nodes(&ctx.db, &run.id, &recipe.flow)
            .await
            .expect("sync");
        store::mark_node_path(
            &ctx.db,
            &run.id,
            "decidir/t1",
            "DONE",
            Some(&json!({ "summary": "cached" })),
            Some("session-1"),
            None,
        )
        .await
        .expect("cache");

        execute(
            &recipe_context(&ctx),
            &recipe,
            &run,
            &json!({}),
            &crate::recipe_dispatch::NoopRecipeDispatcher,
        )
        .await
        .expect("execute");

        let node = store::node_by_path(&ctx.db, &run.id, "decidir/t1")
            .await
            .expect("node")
            .expect("row");
        assert_eq!(node.status, "DONE");
        assert_eq!(node.attempts, 0, "resume must not re-run a DONE step");
        let completed = store::get_run(&ctx.db, &run.id).await.expect("run");
        let outputs: Value = serde_json::from_str(&completed.step_outputs_json).expect("outputs");
        assert_eq!(outputs["t1"]["summary"], "cached");
    }
}
