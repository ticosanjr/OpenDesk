use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    opencode_client::rest_client::ModelRefDto,
};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub async fn model_health(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let entries = crate::db::model_health::list(&state.db, 24).await?;
    Ok(Json(serde_json::json!({ "models": entries })))
}

#[derive(Debug, serde::Deserialize)]
pub struct ClearModelHealthBody {
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
}

pub async fn clear_model_health(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ClearModelHealthBody>,
) -> AppResult<Json<Value>> {
    if let (Some(provider_id), Some(model_id)) = (body.provider_id, body.model_id) {
        crate::db::model_health::clear_model(&state.db, &provider_id, &model_id).await?;
    } else {
        crate::db::model_health::clear(&state.db).await?;
    }
    model_health(State(state)).await
}

#[derive(Debug, Deserialize)]
pub struct TestModelBody {
    pub model: String,
    pub workspace_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ModelTestResult {
    pub ok: bool,
    pub model: String,
    pub provider: Option<String>,
    pub variant: Option<String>,
    pub latency_ms: i64,
    pub response: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

/// Performs one stateless generation with the selected model so the UI can
/// distinguish engine connectivity from provider quota/auth failures.
pub async fn test_model(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TestModelBody>,
) -> AppResult<Json<ModelTestResult>> {
    let model = ModelRefDto::parse(&body.model)
        .ok_or_else(|| AppError::BadRequest(format!("modelo inválido '{}'", body.model)))?;
    let model_label = format!("{}/{}", model.provider_id, model.id);
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    let templates =
        crate::dsh_router::prompt_templates::PromptTemplates::load(&state.db, Some(&workspace_id))
            .await
            .unwrap_or_default();
    let model_test_prompt = crate::dsh_router::prompt_templates::render(&templates.model_test, &[]);
    let Some(client) = state.daemon.client().await else {
        return Ok(Json(ModelTestResult {
            ok: false,
            model: model_label,
            provider: Some(model.provider_id),
            variant: model.variant,
            latency_ms: 0,
            response: None,
            error_code: Some("engine_offline".into()),
            error_message: Some("O motor OpenCode não está online.".into()),
        }));
    };

    let started = Instant::now();
    let failure = |code: &str, message: String| ModelTestResult {
        ok: false,
        model: model_label.clone(),
        provider: Some(model.provider_id.clone()),
        variant: model.variant.clone(),
        latency_ms: started.elapsed().as_millis() as i64,
        response: None,
        error_code: Some(code.to_string()),
        error_message: Some(message),
    };

    let result = tokio::time::timeout(
        Duration::from_secs(45),
        client.generate_text(&model_test_prompt, Some(model.clone())),
    )
    .await;

    let result = match result {
        Err(_) => failure(
            "timeout",
            "tempo esgotado aguardando a resposta mínima do modelo".into(),
        ),
        Ok(Err(error)) => {
            let message = format!("{error:#}");
            let code = classify_model_error(&message);
            if matches!(code, "provider_quota" | "rate_limited") {
                let _ = crate::db::model_health::record_cooldown(&state.db, &model_label, code, 60)
                    .await;
            }
            failure(code, message)
        }
        Ok(Ok(text)) => ModelTestResult {
            ok: true,
            model: model_label,
            provider: Some(model.provider_id),
            variant: model.variant,
            latency_ms: started.elapsed().as_millis() as i64,
            response: Some(text),
            error_code: None,
            error_message: None,
        },
    };
    Ok(Json(result))
}

pub(super) fn classify_model_error(message: &str) -> &'static str {
    let lower = message.to_lowercase();
    if lower.contains("provider.quota")
        || lower.contains("insufficient balance")
        || lower.contains("status\":402")
        || lower.contains("status\": 402")
    {
        "provider_quota"
    } else if lower.contains("rate_limit")
        || lower.contains("rate limit")
        || lower.contains("status\":429")
        || lower.contains("status\": 429")
        || lower.contains("429")
    {
        "rate_limited"
    } else if lower.contains("401") || lower.contains("unauthorized") {
        "auth_failed"
    } else if lower.contains("modelnotfound")
        || lower.contains("model not found")
        || lower.contains("unknown model")
    {
        "model_not_found"
    } else if lower.contains("404") {
        "engine_unsupported"
    } else if lower.contains("timeout") || lower.contains("timed out") {
        "timeout"
    } else {
        "provider_error"
    }
}
