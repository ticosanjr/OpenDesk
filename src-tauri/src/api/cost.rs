use crate::{app_state::AppState, error::AppResult};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct CostQuery {
    pub workspace_id: Option<String>,
    pub days: Option<i64>,
}

pub async fn model_mix(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CostQuery>,
) -> AppResult<Json<serde_json::Value>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let days = query.days.unwrap_or(7).clamp(1, 90);
    let rows = sqlx::query_as::<_, (String, String, i64, i64, f64)>(
        "SELECT provider_id, model_id, COUNT(*),
                COALESCE(SUM(tokens_input + tokens_output), 0),
                COALESCE(SUM(estimated_cost_usd), 0.0)
         FROM task_runs
         WHERE workspace_id = ? AND started_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?)
           AND provider_id IS NOT NULL
         GROUP BY provider_id, model_id
         ORDER BY 4 DESC",
    )
    .bind(&workspace_id)
    .bind(format!("-{days} days"))
    .fetch_all(&state.db)
    .await?;

    let total_tokens: i64 = rows.iter().map(|row| row.3).sum();
    let total_cost: f64 = rows.iter().map(|row| row.4).sum();
    let max_pptoken = rows
        .iter()
        .filter(|row| row.3 > 0)
        .map(|row| row.4 / row.3 as f64)
        .fold(0.0f64, f64::max);
    let models = rows
        .iter()
        .map(|(provider, model, runs, tokens, cost)| {
            serde_json::json!({
                "provider_id": provider,
                "model_id": model,
                "runs": runs,
                "tokens": tokens,
                "cost_usd": cost,
                "percent": if total_tokens > 0 { (*tokens as f64 / total_tokens as f64) * 100.0 } else { 0.0 },
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(serde_json::json!({
        "days": days,
        "models": models,
        "total_tokens": total_tokens,
        "total_cost_usd": total_cost,
        "baseline_cost_usd": max_pptoken * total_tokens as f64,
        "savings_usd": (max_pptoken * total_tokens as f64 - total_cost).max(0.0),
    })))
}
