use crate::{
    engine::finish,
    error::{AppError, AppResult},
    parser, store,
};
use futures_util::stream::{self, StreamExt};
use serde_json::{Value, json};

use super::executor::run_scope;
use super::nodes::{enforce_budget, ensure_node, finished, liveness};
use super::paths::{child_path, item_key};
use super::types::{Ctx, MapItemResult, ScopeOutcome, ScopeState};

pub(super) async fn run_map(
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
