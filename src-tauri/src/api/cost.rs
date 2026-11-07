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
    let rows = sqlx::query_as::<_, (String, String, i64, i64, f64, i64, i64)>(
        "SELECT provider_id, model_id, COUNT(*),
                COALESCE(SUM(tokens_input + tokens_output), 0),
                COALESCE(SUM(estimated_cost_usd), 0.0),
                COALESCE(SUM(tokens_cache_read), 0),
                COALESCE(SUM(tokens_input), 0)
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
        .map(|(provider, model, runs, tokens, cost, cache_read, input)| {
            let cache_ratio = if *input + *cache_read > 0 {
                *cache_read as f64 / (*input + *cache_read) as f64
            } else {
                0.0
            };
            serde_json::json!({
                "provider_id": provider,
                "model_id": model,
                "runs": runs,
                "tokens": tokens,
                "cost_usd": cost,
                "cache_read": *cache_read,
                "cache_ratio": cache_ratio,
                "percent": if total_tokens > 0 { (*tokens as f64 / total_tokens as f64) * 100.0 } else { 0.0 },
            })
        })
        .collect::<Vec<_>>();

    let cache_read: i64 = rows.iter().map(|row| row.5).sum();
    let cache_input: i64 = rows.iter().map(|row| row.6).sum();
    let cache_ratio = if cache_input + cache_read > 0 {
        cache_read as f64 / (cache_input + cache_read) as f64
    } else {
        0.0
    };

    Ok(Json(serde_json::json!({
        "days": days,
        "cache_read": cache_read,
        "cache_ratio": cache_ratio,
        "models": models,
        "total_tokens": total_tokens,
        "total_cost_usd": total_cost,
        "baseline_cost_usd": max_pptoken * total_tokens as f64,
        "savings_usd": (max_pptoken * total_tokens as f64 - total_cost).max(0.0),
    })))
}
