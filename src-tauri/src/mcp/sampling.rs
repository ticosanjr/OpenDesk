//! Server-side sampling fallback (E5).
//!
//! MCP clients may implement `sampling/createMessage`; when they do, the
//! gateway proxies the request over the session. When they do not, OpenDesk
//! answers through the same DSH router used by tasks, after the budget check.

use crate::{
    app_state::AppState,
    db::{budget, router as router_db, workspaces},
    dsh_router::{self, RouterInput},
    error::AppError,
};
use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};

pub async fn fallback(state: &AppState, workspace_id: &str, args: &Value) -> Result<Value> {
    let check = budget::check(&state.db)
        .await
        .map_err(|error| anyhow!("{error:#}"))?;
    if !check.allowed {
        bail!(check.message);
    }
    let prompt = args
        .get("prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("sampling.prompt é obrigatório"))?;
    let workspace = workspaces::get(&state.db, workspace_id).await?;
    let models = state
        .daemon
        .models(Some(&workspace.root_path))
        .await
        .map_err(|error| anyhow!("{error:#}"))?;
    if models.is_empty() {
        bail!("nenhum modelo disponível para sampling");
    }
    let rules = router_db::list(&state.db, workspace_id)
        .await
        .map_err(|error| anyhow!("{error:#}"))?;
    let status = budget::status(&state.db)
        .await
        .map_err(|error| anyhow!("{error:#}"))?;
    let manual_model = args.get("model").and_then(Value::as_str);
    let input = RouterInput {
        workspace_id,
        workspace_root: std::path::Path::new(&workspace.root_path),
        title: "MCP sampling",
        prompt,
        execution_mode: "BUILD_DIRECT",
        priority: 2,
        tags: &[],
        manual_agent: Some("build"),
        manual_model,
        manual_effort: None,
        manual_harness: Some("opencode"),
        default_harness: Some("opencode"),
        policy: &workspace.default_router_policy,
        rules: &rules,
        available_models: &models,
        default_model: None,
        manual_default_model: None,
        approved_plan: None,
        output_directory: None,
        budget_daily_pct: status.daily_pct,
    };
    let decision = dsh_router::classify(&input);
    let client = state
        .daemon
        .client()
        .await
        .ok_or_else(|| AppError::Daemon("OpenCode daemon is not online".into()))?;
    let model = crate::opencode_client::rest_client::ModelRefDto::parse(&decision.selected_model);
    let text = client
        .generate_text(prompt, model)
        .await
        .map_err(|error| anyhow!("{error:#}"))?;
    Ok(json!({
        "model": decision.selected_model,
        "text": text,
        "budget": { "daily_pct": status.daily_pct, "hard_stop": status.hard_stop },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn refuses_when_budget_hard_stop_is_reached() {
        let ctx = crate::test_support::context().await;
        budget::update(
            &ctx.state.db,
            budget::UpdateBudgetInput {
                daily_limit_usd: Some(0.0001),
                monthly_limit_usd: Some(0.0001),
                soft_alert_threshold_pct: None,
                hard_stop_enabled: Some(true),
                opencode_go_5h_token_limit: None,
            },
        )
        .await
        .expect("budget");
        sqlx::query(
            "INSERT INTO task_runs
                (id, workspace_id, agent_name, execution_mode, model_id, effort_level,
                 status, estimated_cost_usd)
             VALUES ('run-budget', ?, 'build', 'BUILD_DIRECT', 'model', 'medium',
                 'SUCCESS', 1.0)",
        )
        .bind(&ctx.workspace_id)
        .execute(&ctx.state.db)
        .await
        .expect("run");
        let error = fallback(&ctx.state, &ctx.workspace_id, &json!({ "prompt": "oi" }))
            .await
            .expect_err("budget");
        assert!(format!("{error}").to_lowercase().contains("hard cap"));
    }
}
