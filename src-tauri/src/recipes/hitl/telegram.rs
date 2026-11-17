use crate::{
    error::{AppError, AppResult},
    integrations::telegram,
    recipe_context::RecipeContext,
    recipes::hitl::{HitlRequestPayload, parse_reply, resolve},
    recipes::store,
};

pub async fn dispatch(ctx: &RecipeContext, payload: &HitlRequestPayload) -> AppResult<String> {
    let config = telegram::load(&ctx.pool).await?;
    if !config.enabled || telegram::token(&ctx.secrets).is_none() || config.allowlist.is_empty() {
        return Err(AppError::BadRequest(
            "Telegram não está habilitado ou sem allowlist".into(),
        ));
    }
    let text = format!(
        "[OD-#{}] {}\n\n{}\n\n{}",
        payload.token,
        payload.title,
        payload.message_body,
        crate::recipes::hitl::reply_instructions(&payload.interaction_type, &payload.options)
    );
    let sent = telegram::send_to_allowlist_parts(&ctx.pool, &ctx.secrets, &text).await?;
    if sent == 0 {
        return Err(AppError::BadRequest(
            "nenhum chat conhecido na allowlist do Telegram".into(),
        ));
    }
    Ok(format!(
        "telegram-{}",
        chrono::Utc::now().timestamp_millis()
    ))
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
        let options: Vec<String> = serde_json::from_str(&request.options_json).unwrap_or_default();
        parse_reply(&request.interaction_type, &options, text)
    };
    resolve(
        ctx,
        &token,
        "TELEGRAM",
        Some(sender),
        &kind,
        payload.as_deref(),
    )
    .await?;
    if let Ok(bot_token) = telegram::token_for_parts(&ctx.secrets) {
        let _ = telegram::send_text(&bot_token, sender, "OpenDesk: resposta recebida.").await;
    }
    Ok(true)
}
