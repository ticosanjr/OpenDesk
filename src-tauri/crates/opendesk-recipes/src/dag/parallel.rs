use crate::{
    engine::finish,
    error::{AppError, AppResult},
    parser, store,
};
use futures_util::future::join_all;
use serde_json::json;

use super::executor::run_scope;
use super::nodes::{
    enforce_budget, ensure_node, finished, liveness, merge_outputs, restore_finished,
};
use super::paths::child_path;
use super::types::{Ctx, ScopeOutcome, ScopeState};

pub(super) async fn run_parallel(
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
