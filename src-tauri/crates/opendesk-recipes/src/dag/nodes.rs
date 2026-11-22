use crate::{
    error::{AppError, AppResult},
    store,
};
use serde_json::Value;

use super::types::{Ctx, ScopeState};

pub(super) async fn restore_finished(
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

pub(super) async fn account_step(
    ctx: &Ctx<'_>,
    scope: &mut ScopeState,
    step_id: &str,
) -> AppResult<()> {
    let run_id = scope
        .outputs
        .get(step_id)
        .and_then(|value| value.get("run_id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(run_id) = run_id
        && let Ok(run) = opendesk_storage::repos::runs::get(&ctx.context.pool, &run_id).await
    {
        scope.spent_usd += run.estimated_cost_usd.max(0.0);
    }
    enforce_budget(scope)
}

pub(super) fn enforce_budget(scope: &ScopeState) -> AppResult<()> {
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

pub(super) fn merge_node_output(
    scope: &mut ScopeState,
    node: &store::RecipeRunNode,
    step_id: &str,
) {
    if let Some(output) = parse_output(node) {
        scope.outputs[step_id] = output;
    }
}

pub(super) async fn ensure_node(
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

pub(super) fn finished(node: &store::RecipeRunNode) -> bool {
    matches!(node.status.as_str(), "DONE" | "SKIPPED")
}

pub(super) async fn liveness(ctx: &Ctx<'_>, node_id: &str, outputs: &Value) -> AppResult<()> {
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

pub(super) fn merge_outputs(target: &mut Value, source: &Value) {
    if let (Some(target), Some(source)) = (target.as_object_mut(), source.as_object()) {
        for (key, value) in source {
            target.insert(key.clone(), value.clone());
        }
    }
}
