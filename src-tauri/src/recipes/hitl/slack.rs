use crate::{
    error::{AppError, AppResult},
    recipe_context::RecipeContext,
    recipes::hitl::HitlRequestPayload,
};
use opendesk_channels::slack;

pub async fn dispatch(ctx: &RecipeContext, payload: &HitlRequestPayload) -> AppResult<String> {
    let config = slack::load(&ctx.pool).await?;
    let token = slack::bot_token(&ctx.secrets)
        .ok_or_else(|| AppError::BadRequest("bot token do Slack ausente".into()))?;
    let channel = config
        .channel_allowlist
        .first()
        .ok_or_else(|| AppError::BadRequest("configure um canal no allowlist do Slack".into()))?;
    let text = format!("{}\n\n{}", payload.title, payload.message_body);
    let actions = vec![
        (
            format!("hitl:{}:APPROVED", payload.token),
            "Aprovar".to_string(),
        ),
        (
            format!("hitl:{}:REJECTED", payload.token),
            "Rejeitar".to_string(),
        ),
    ];
    slack::send_hitl(&token, channel, &payload.title, &text, &actions).await?;
    Ok(format!("slack-{}", chrono::Utc::now().timestamp_millis()))
}
