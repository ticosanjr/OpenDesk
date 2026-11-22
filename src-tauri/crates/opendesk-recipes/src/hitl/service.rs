use crate::context::{HitlChannelRequest, RecipeContext};
use crate::parser::HitlCheckpoint;
use crate::store::{self, HitlRequest, RecipeRun};

use super::replies::parse_resolution;
use super::types::{HitlOutcome, HitlResolution};

use crate::error::AppError;
use crate::error::AppResult;
use chrono::{Duration, Utc};
use rand::RngCore;

pub async fn request(
    ctx: &RecipeContext,
    run: &RecipeRun,
    checkpoint: &HitlCheckpoint,
    title: &str,
    message: &str,
    next_step: Option<&str>,
    attachments: &[String],
) -> AppResult<String> {
    let token = unique_token(&ctx.pool).await?;
    let expires_at =
        (Utc::now() + Duration::minutes(checkpoint.timeout_minutes.clamp(1, 10080))).to_rfc3339();
    let context = serde_json::json!({
        "recipe_name": title,
        "recipe_run_id": run.id,
        "task_id": run.task_id,
        "checkpoint_id": checkpoint.id,
        "interaction_type": checkpoint.interaction_type,
        "on_timeout": checkpoint.on_timeout,
        "timeout_minutes": checkpoint.timeout_minutes,
        "channels": checkpoint.channels,
        "attachments": attachments,
        "next_step": next_step,
    });
    store::create_hitl(
        &ctx.pool,
        &token,
        run,
        &checkpoint.id,
        &checkpoint.interaction_type,
        title,
        message,
        &context,
        &checkpoint.options,
        &checkpoint.on_timeout,
        &expires_at,
    )
    .await?;
    let payload = HitlChannelRequest {
        token: token.clone(),
        run_id: run.id.clone(),
        workspace_id: run.workspace_id.clone(),
        interaction_type: checkpoint.interaction_type.clone(),
        title: title.to_string(),
        message_body: message.to_string(),
        options: checkpoint.options.clone(),
        attachments: attachments.to_vec(),
    };
    for channel in &checkpoint.channels {
        let result = if channel == "DESKTOP_UI" {
            Ok("desktop".to_string())
        } else {
            ctx.channels().dispatch(ctx, channel, &payload).await
        };
        match result {
            Ok(external) => {
                let _ = store::dispatch_channel(&ctx.pool, &token, channel, &external).await;
            }
            Err(error) => {
                tracing::warn!(token = %token, channel = %channel, error = %error, "HITL dispatch failed");
            }
        }
    }
    Ok(token)
}

pub async fn wait(
    ctx: &RecipeContext,
    token: &str,
    timeout_minutes: i64,
) -> AppResult<HitlOutcome> {
    let deadline = Utc::now() + Duration::minutes(timeout_minutes.clamp(1, 10080));
    loop {
        let request = store::get_hitl(&ctx.pool, token).await?;
        match request.status.as_str() {
            "RESOLVED" => {
                let resolution = request
                    .resolution_type
                    .as_deref()
                    .map(parse_resolution)
                    .unwrap_or(HitlResolution::Approved);
                return Ok(HitlOutcome::Resolved(resolution));
            }
            "CANCELLED" => return Ok(HitlOutcome::Cancelled),
            "EXPIRED" => return Ok(HitlOutcome::Expired),
            _ => {}
        }
        if Utc::now() >= deadline {
            let _ = store::resolve_hitl(&ctx.pool, token, "TIMEOUT", None, "EXPIRED", None).await;
            return Ok(HitlOutcome::Expired);
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

pub async fn resolve(
    ctx: &RecipeContext,
    token: &str,
    channel: &str,
    sender: Option<&str>,
    resolution_type: &str,
    payload: Option<&str>,
) -> AppResult<Option<HitlRequest>> {
    let resolved =
        store::resolve_hitl(&ctx.pool, token, channel, sender, resolution_type, payload).await?;
    if resolved.is_some() {
        ctx.events.publish(
            opendesk_storage::events::OpenDeskWsEvent::RecipeRunUpdated {
                run_id: resolved
                    .as_ref()
                    .map(|request| request.recipe_run_id.clone())
                    .unwrap_or_default(),
                recipe_id: String::new(),
                workspace_id: resolved
                    .as_ref()
                    .map(|request| request.workspace_id.clone())
                    .unwrap_or_default(),
                status: "HITL_RESOLVED".into(),
            },
        );
    }
    Ok(resolved)
}

async fn unique_token(pool: &sqlx::SqlitePool) -> AppResult<String> {
    for _ in 0..32 {
        let mut bytes = [0u8; 2];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        let token = format!("{:02X}{:02X}", bytes[0], bytes[1]);
        if sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM hitl_requests WHERE token = ?")
            .bind(&token)
            .fetch_one(pool)
            .await?
            == 0
        {
            return Ok(token);
        }
    }
    Err(AppError::Internal(
        "não foi possível gerar token HITL".into(),
    ))
}
