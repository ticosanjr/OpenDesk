use crate::{error::AppResult, parser, store};
use serde_json::json;

use super::executor::run_scope;
use super::nodes::{ensure_node, finished};
use super::paths::child_path;
use super::types::{Ctx, ScopeOutcome, ScopeState};

pub(super) async fn run_switch(
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

pub(super) async fn run_loop(
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
