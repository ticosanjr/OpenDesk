use crate::{
    error::{AppError, AppResult},
    recipe_context::RecipeContext,
    recipes::hitl::HitlRequestPayload,
};
use opendesk_channels::discord;

pub async fn dispatch(ctx: &RecipeContext, payload: &HitlRequestPayload) -> AppResult<String> {
    let config = discord::load(&ctx.pool).await?;
    let token = discord::token(&ctx.secrets)
        .ok_or_else(|| AppError::BadRequest("token do Discord ausente".into()))?;
    let channel = config
        .channel_allowlist
        .first()
        .ok_or_else(|| AppError::BadRequest("configure um canal no allowlist do Discord".into()))?;
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
    discord::send_hitl(&token, channel, &payload.title, &text, &actions).await?;
    Ok(format!("discord-{}", chrono::Utc::now().timestamp_millis()))
}
