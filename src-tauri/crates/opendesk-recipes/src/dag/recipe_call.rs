use crate::{
    error::{AppError, AppResult},
    parser, store,
};
use serde_json::{Value, json};

use super::executor::run_scope;
use super::nodes::{enforce_budget, ensure_node, finished, liveness, restore_finished};
use super::paths::child_path;
use super::types::{Ctx, ScopeOutcome, ScopeState};

pub(super) async fn run_recipe_call(
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
