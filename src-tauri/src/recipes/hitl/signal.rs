use crate::{
    error::{AppError, AppResult},
    integrations::signal,
    recipe_context::RecipeContext,
    recipes::hitl::{HitlRequestPayload, parse_reply, resolve},
    recipes::store,
};

pub async fn dispatch(ctx: &RecipeContext, payload: &HitlRequestPayload) -> AppResult<String> {
    let config = signal::load(&ctx.pool).await?;
    if !config.enabled || config.account.trim().is_empty() || config.allowlist.is_empty() {
        return Err(AppError::BadRequest(
            "Signal não está habilitado ou sem allowlist".into(),
        ));
    }
    let text = format!(
        "[OD-#{}] {}\n\n{}\n\n{}",
        payload.token,
        payload.title,
        payload.message_body,
        crate::recipes::hitl::reply_instructions(&payload.interaction_type, &payload.options)
    );
    for recipient in &config.allowlist {
        signal::send_with_attachments(
            &config.cli_path,
            &config.account,
            recipient,
            &text,
            &payload.attachments,
        )
        .await?;
    }
    Ok(format!("signal-{}", chrono::Utc::now().timestamp_millis()))
}

pub async fn handle_reply(
    ctx: &RecipeContext,
    sender: &str,
    text: &str,
    attachments: &[String],
) -> AppResult<bool> {
    let Some(token) = crate::recipes::hitl::extract_token(text) else {
        return Ok(false);
    };
    let Ok(request) = store::get_hitl(&ctx.pool, &token).await else {
        return Ok(false);
    };
    if request.status != "PENDING" {
        return Ok(true);
    }
    let (kind, payload) = if let Some(path) = attachments.first() {
        (
            "ARTIFACT".to_string(),
            Some(serde_json::json!({ "path": path }).to_string()),
        )
    } else {
        {
            let options: Vec<String> =
                serde_json::from_str(&request.options_json).unwrap_or_default();
            parse_reply(&request.interaction_type, &options, text)
        }
    };
    resolve(
        ctx,
        &token,
        "SIGNAL",
        Some(sender),
        &kind,
        payload.as_deref(),
    )
    .await?;
    let config = signal::load(&ctx.pool).await?;
    let _ = signal::send_text(
        &config.cli_path,
        &config.account,
        sender,
        "OpenDesk: resposta recebida.",
    )
    .await;
    Ok(true)
}
