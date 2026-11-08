use crate::{
    app_state::AppState,
    db::{
        budget,
        router::{self, CreateRuleInput, RouterRule, UpdateRuleInput},
    },
    error::{AppError, AppResult},
    opencode_client::rest_client::ModelRefDto,
};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct RouterQuery {
    pub workspace_id: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct RouterConfig {
    pub workspace_id: String,
    pub policy: String,
    pub router_model: Option<String>,
    pub cost_blend: f64,
    pub rules: Vec<RouterRule>,
    pub budget: budget::BudgetStatus,
}

pub async fn get_config(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RouterQuery>,
) -> AppResult<Json<RouterConfig>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let policy = router::workspace_policy(&state.db, &workspace_id).await?;
    let rules = router::list(&state.db, &workspace_id).await?;
    let budget = budget::status(&state.db).await?;
    let router_model = crate::db::settings::get(&state.db, "router_model")
        .await?
        .and_then(|value| value.as_str().map(str::to_string));
    let cost_blend = cost_blend(&state.db).await?;
    Ok(Json(RouterConfig {
        workspace_id,
        policy,
        router_model,
        cost_blend,
        rules,
        budget,
    }))
}

pub async fn list_rules(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RouterQuery>,
) -> AppResult<Json<Vec<RouterRule>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(router::list(&state.db, &workspace_id).await?))
}

fn validate_model(value: &str) -> AppResult<()> {
    let value = value.trim();
    if !value.is_empty() && ModelRefDto::parse(value).is_none() {
        return Err(AppError::BadRequest(format!(
            "modelo inválido '{value}'; use provider/modelo ou provider/modelo#variant"
        )));
    }
    Ok(())
}

pub async fn create_rule(
    State(state): State<Arc<AppState>>,
    Json(input): Json<CreateRuleInput>,
) -> AppResult<Json<RouterRule>> {
    validate_model(&input.target_model)?;
    Ok(Json(router::create(&state.db, input).await?))
}

pub async fn update_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(input): Json<UpdateRuleInput>,
) -> AppResult<Json<RouterRule>> {
    if let Some(model) = input.target_model.as_deref() {
        validate_model(model)?;
    }
    Ok(Json(router::update(&state.db, &id, input).await?))
}

pub async fn delete_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    router::delete(&state.db, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub struct RouterModelBody {
    pub model: Option<String>,
}

/// Persist the model used for advisory cost comparisons. The deterministic
/// router keeps working even when this is unset.
pub async fn set_router_model(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RouterModelBody>,
) -> AppResult<Json<serde_json::Value>> {
    let model = body
        .model
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(model) = model {
        validate_model(model)?;
    }
    let value = model
        .map(|model| serde_json::Value::String(model.to_string()))
        .unwrap_or(serde_json::Value::Null);
    crate::db::settings::set(&state.db, "router_model", &value).await?;
    Ok(Json(serde_json::json!({ "router_model": model })))
}

#[derive(Debug, Deserialize, Default)]
pub struct AnalyzeBody {
    pub workspace_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ModelCost {
    pub model: String,
    pub provider: String,
    pub name: String,
    pub input_cost: f64,
    pub output_cost: f64,
    pub blended_cost: f64,
}

/// Compare the models currently exposed by the daemon by their per-million
/// token prices. This is computed locally; no LLM call is required.
pub async fn analyze(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AnalyzeBody>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    let workspace = crate::db::workspaces::get(&state.db, &workspace_id).await?;
    let models = state
        .daemon
        .models(Some(&workspace.root_path))
        .await
        .map_err(|error| {
            AppError::BadRequest(format!("não foi possível listar modelos: {error}"))
        })?;
    let blend = cost_blend(&state.db).await?;
    let mut rows = Vec::new();
    for model in models {
        let Some((input_cost, output_cost)) = model_cost(&model.cost) else {
            continue;
        };
        let provider = model.provider_id.clone().unwrap_or_default();
        let id = model.model_id.clone().unwrap_or_else(|| model.id.clone());
        rows.push(ModelCost {
            model: if provider.is_empty() {
                id.clone()
            } else {
                format!("{provider}/{id}")
            },
            provider,
            name: model.name.clone().unwrap_or(id),
            input_cost,
            output_cost,
            blended_cost: blended_cost(input_cost, output_cost, blend),
        });
    }
    rows.sort_by(|left, right| {
        left.blended_cost
            .partial_cmp(&right.blended_cost)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let cheapest = rows.first().map(|row| row.model.clone());
    let router_model = crate::db::settings::get(&state.db, "router_model")
        .await?
        .and_then(|value| value.as_str().map(str::to_string));
    let recommendation = if rows.is_empty() {
        "Nenhum modelo com preço disponível. Verifique se o OpenCode está online e expõe custos por modelo.".to_string()
    } else {
        let cheapest = rows.first().expect("non-empty");
        let priciest = rows.last().expect("non-empty");
        let spread = if cheapest.blended_cost > 0.0 {
            priciest.blended_cost / cheapest.blended_cost
        } else {
            1.0
        };
        format!(
            "Com {:.0}% de peso na entrada, o menor custo combinado é {} (${:.3}/1M). O mais caro é {} (${:.3}/1M), {:.1}x maior. {} modelo(s) comparados.{}",
            blend * 100.0,
            cheapest.model,
            cheapest.blended_cost,
            priciest.model,
            priciest.blended_cost,
            spread,
            rows.len(),
            router_model
                .as_deref()
                .map(|model| format!(
                    " Modelo roteador configurado: {model} (parecer local, sem chamada ao modelo)."
                ))
                .unwrap_or_default()
        )
    };
    Ok(Json(serde_json::json!({
        "workspace_id": workspace_id,
        "router_model": router_model,
        "cost_blend": blend,
        "models": rows,
        "cheapest": cheapest,
        "recommendation": recommendation,
        "generated_at": chrono::Utc::now().to_rfc3339(),
    })))
}

fn model_cost(costs: &[serde_json::Value]) -> Option<(f64, f64)> {
    let first = costs.first()?;
    let input = first.get("input").and_then(serde_json::Value::as_f64)?;
    let output = first.get("output").and_then(serde_json::Value::as_f64)?;
    Some((input, output))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_changes_the_ranking_weight() {
        assert!((blended_cost(1.0, 3.0, 0.5) - 2.0).abs() < f64::EPSILON);
        assert!((blended_cost(1.0, 3.0, 0.7) - 1.6).abs() < f64::EPSILON);
    }

    #[test]
    fn reads_per_million_costs_from_daemon_payload() {
        let costs = vec![serde_json::json!({
            "input": 0.25,
            "output": 1.5,
            "cache": { "read": 0.05, "write": 0.3 }
        })];
        assert_eq!(model_cost(&costs), Some((0.25, 1.5)));
        assert_eq!(model_cost(&[]), None);
        assert_eq!(model_cost(&[serde_json::json!({ "input": 1.0 })]), None);
    }
}

async fn cost_blend(pool: &sqlx::SqlitePool) -> AppResult<f64> {
    let value = crate::db::settings::get(pool, "router_cost_blend")
        .await?
        .and_then(|value| value.as_f64())
        .unwrap_or(0.7);
    Ok(value.clamp(0.0, 1.0))
}

#[derive(Debug, Deserialize)]
pub struct BlendBody {
    pub blend: f64,
}

pub async fn set_blend(
    State(state): State<Arc<AppState>>,
    Json(body): Json<BlendBody>,
) -> AppResult<Json<serde_json::Value>> {
    if !(0.0..=1.0).contains(&body.blend) {
        return Err(AppError::BadRequest(
            "blend precisa estar entre 0 e 1".into(),
        ));
    }
    crate::db::settings::set(
        &state.db,
        "router_cost_blend",
        &serde_json::json!(body.blend),
    )
    .await?;
    Ok(Json(serde_json::json!({ "blend": body.blend })))
}

fn blended_cost(input: f64, output: f64, blend: f64) -> f64 {
    input * blend + output * (1.0 - blend)
}
