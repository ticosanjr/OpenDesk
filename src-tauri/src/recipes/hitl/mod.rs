use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    recipes::{
        parser::HitlCheckpoint,
        store::{self, HitlRequest, RecipeRun},
    },
};
use chrono::{Duration, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod email;
pub mod signal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HitlResolution {
    Approved,
    Rejected {
        reason: Option<String>,
    },
    ChoiceSelected {
        option_index: usize,
        option_value: String,
    },
    FeedbackProvided {
        feedback_text: String,
        attached_files: Vec<String>,
    },
    ArtifactEdited {
        path: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitlRequestPayload {
    pub token: String,
    pub run_id: String,
    pub workspace_id: String,
    pub interaction_type: String,
    pub title: String,
    pub message_body: String,
    pub options: Vec<String>,
    pub attachments: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum HitlOutcome {
    Resolved(HitlResolution),
    Expired,
}

pub async fn request(
    state: &Arc<AppState>,
    run: &RecipeRun,
    checkpoint: &HitlCheckpoint,
    title: &str,
    message: &str,
    attachments: &[String],
) -> AppResult<String> {
    let token = unique_token(&state.db).await?;
    let expires_at =
        (Utc::now() + Duration::minutes(checkpoint.timeout_minutes.clamp(1, 10080))).to_rfc3339();
    store::create_hitl(
        &state.db,
        &token,
        run,
        &checkpoint.id,
        &checkpoint.interaction_type,
        &checkpoint.on_timeout,
        &expires_at,
    )
    .await?;
    let payload = HitlRequestPayload {
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
        let result = match channel.as_str() {
            "DESKTOP_UI" => Ok("desktop".to_string()),
            "EMAIL" => email::dispatch(state, &payload).await,
            "SIGNAL" => signal::dispatch(state, &payload).await,
            other => Err(AppError::BadRequest(format!(
                "canal HITL desconhecido: {other}"
            ))),
        };
        match result {
            Ok(external) => {
                let _ = store::dispatch_channel(&state.db, &token, channel, &external).await;
            }
            Err(error) => {
                tracing::warn!(token = %token, channel = %channel, error = %error, "HITL dispatch failed");
            }
        }
    }
    Ok(token)
}

pub async fn wait(
    state: &Arc<AppState>,
    token: &str,
    timeout_minutes: i64,
) -> AppResult<HitlOutcome> {
    let deadline = Utc::now() + Duration::minutes(timeout_minutes.clamp(1, 10080));
    loop {
        let request = store::get_hitl(&state.db, token).await?;
        match request.status.as_str() {
            "RESOLVED" => {
                let resolution = request
                    .resolution_type
                    .as_deref()
                    .map(parse_resolution)
                    .unwrap_or(HitlResolution::Approved);
                return Ok(HitlOutcome::Resolved(resolution));
            }
            "EXPIRED" | "CANCELLED" => return Ok(HitlOutcome::Expired),
            _ => {}
        }
        if Utc::now() >= deadline {
            let _ = store::resolve_hitl(&state.db, token, "TIMEOUT", None, "EXPIRED", None).await;
            return Ok(HitlOutcome::Expired);
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

pub async fn resolve(
    state: &Arc<AppState>,
    token: &str,
    channel: &str,
    sender: Option<&str>,
    resolution_type: &str,
    payload: Option<&str>,
) -> AppResult<Option<HitlRequest>> {
    let resolved =
        store::resolve_hitl(&state.db, token, channel, sender, resolution_type, payload).await?;
    if resolved.is_some() {
        state
            .events
            .publish(crate::events::OpenDeskWsEvent::RecipeRunUpdated {
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
            });
    }
    Ok(resolved)
}

pub fn parse_reply(
    interaction_type: &str,
    options: &[String],
    text: &str,
) -> (String, Option<String>) {
    let clean = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('>'))
        .collect::<Vec<_>>()
        .join("\n");
    let lower = clean.to_lowercase();
    if matches!(
        lower.as_str(),
        "aprovar" | "aprovado" | "sim" | "ok" | "approve" | "yes" | "👍"
    ) {
        return ("APPROVED".into(), None);
    }
    if matches!(
        lower.as_str(),
        "rejeitar" | "rejeitado" | "nao" | "não" | "reject" | "no" | "👎"
    ) {
        return ("REJECTED".into(), Some(clean));
    }
    if interaction_type == "MULTI_CHOICE"
        && let Ok(index) = clean.parse::<usize>()
        && index >= 1
        && index <= options.len()
    {
        return (
            "CHOICE".into(),
            Some(
                serde_json::json!({
                    "option_index": index - 1,
                    "option_value": options[index - 1],
                })
                .to_string(),
            ),
        );
    }
    ("FEEDBACK".into(), Some(clean))
}

fn parse_resolution(value: &str) -> HitlResolution {
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(value) {
        if let Some(path) = json.get("path").and_then(|value| value.as_str()) {
            return HitlResolution::ArtifactEdited {
                path: path.to_string(),
            };
        }
        if let Some(text) = json.get("feedback_text").and_then(|value| value.as_str()) {
            return HitlResolution::FeedbackProvided {
                feedback_text: text.to_string(),
                attached_files: json
                    .get("attached_files")
                    .and_then(|value| value.as_array())
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| item.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
            };
        }
        if let Some(index) = json.get("option_index").and_then(|value| value.as_u64()) {
            return HitlResolution::ChoiceSelected {
                option_index: index as usize,
                option_value: json
                    .get("option_value")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_string(),
            };
        }
    }
    match value {
        "REJECTED" => HitlResolution::Rejected { reason: None },
        "FEEDBACK" => HitlResolution::FeedbackProvided {
            feedback_text: String::new(),
            attached_files: vec![],
        },
        _ => HitlResolution::Approved,
    }
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

pub fn extract_token(text: &str) -> Option<String> {
    let upper = text.to_uppercase();
    let marker = upper.find("OD-#").or_else(|| upper.find("#"))?;
    let start = if upper[marker..].starts_with("OD-#") {
        marker + 4
    } else {
        marker + 1
    };
    let token: String = upper[start..]
        .chars()
        .take_while(|character| character.is_ascii_hexdigit())
        .take(8)
        .collect();
    (token.len() >= 4).then_some(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_approval_and_feedback() {
        assert_eq!(parse_reply("BINARY_GATE", &[], "APROVAR").0, "APPROVED");
        assert_eq!(parse_reply("BINARY_GATE", &[], "não").0, "REJECTED");
        assert_eq!(
            parse_reply("APPROVAL_WITH_FEEDBACK", &[], "Refaça a tabela 2").0,
            "FEEDBACK"
        );
        assert_eq!(extract_token("ok [OD-#8F2A]").as_deref(), Some("8F2A"));
    }
}
