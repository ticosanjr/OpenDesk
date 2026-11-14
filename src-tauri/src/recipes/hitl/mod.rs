use crate::{
    app_state::AppState,
    error::{AppError, AppResult},
    hitl::HitlKind,
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
pub mod telegram;

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
    next_step: Option<&str>,
    attachments: &[String],
) -> AppResult<String> {
    let token = unique_token(&state.db).await?;
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
        &state.db,
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
            "TELEGRAM" => telegram::dispatch(state, &payload).await,
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

/// Channel-facing instructions for a checkpoint, in the same vocabulary the
/// Desktop UI uses.
pub fn reply_instructions(interaction_type: &str, options: &[String]) -> String {
    let kind = HitlKind::parse(interaction_type).unwrap_or(HitlKind::Text);
    match kind {
        HitlKind::Approval => {
            "Responda APROVAR ou REJEITAR. Para explicar, escreva APROVAR/REJEITAR seguido do payload."
                .to_string()
        }
        HitlKind::SingleChoice | HitlKind::MultiChoice => {
            let list = options
                .iter()
                .enumerate()
                .map(|(index, option)| format!("{}. {option}", index + 1))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "Responda com o número da opção (ou números separados por vírgula).\n{list}"
            )
        }
        HitlKind::Text => "Responda com o texto desejado na primeira linha.".to_string(),
    }
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
    let kind = HitlKind::parse(interaction_type).unwrap_or(HitlKind::Text);
    if kind.requires_options() {
        let mut selected = Vec::new();
        for part in clean
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            let Ok(index) = part.parse::<usize>() else {
                selected.clear();
                break;
            };
            if index == 0 || index > options.len() {
                selected.clear();
                break;
            }
            selected.push(index - 1);
        }
        if let Some(first) = selected.first() {
            let value = selected
                .iter()
                .map(|index| options[*index].as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return (
                "CHOICE".into(),
                Some(
                    serde_json::json!({
                        "option_index": first,
                        "option_value": value,
                    })
                    .to_string(),
                ),
            );
        }
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
        assert_eq!(extract_token("sem token").as_deref(), None);
        let options = vec!["A".to_string(), "B".to_string()];
        let (kind, payload) = parse_reply("MULTI_CHOICE", &options, "2");
        assert_eq!(kind, "CHOICE");
        assert!(payload.expect("payload").contains("B"));
    }

    use crate::recipes::{parser, store};
    use crate::test_support;
    use serde_json::json;

    fn checkpoint(channels: &[&str]) -> HitlCheckpoint {
        HitlCheckpoint {
            id: "checkpoint-1".into(),
            after_step: String::new(),
            name: Some("Aprovação".into()),
            description: None,
            interaction_type: "BINARY_GATE".into(),
            channels: channels.iter().map(|value| value.to_string()).collect(),
            timeout_minutes: 30,
            on_timeout: "PAUSE_AND_ALERT".into(),
            prompt_template: "Aprovar?".into(),
            attach_files: Vec::new(),
            options: Vec::new(),
        }
    }

    async fn fixture() -> (test_support::TestContext, RecipeRun) {
        let ctx = test_support::context().await;
        let yaml = "id: hitl-recipe\nname: Receita HITL\ntrigger:\n  type: MANUAL\n  config: {}\nflow:\n  - type: step\n    id: s1\n    name: Passo\n    prompt: oi\n";
        let recipe = parser::parse(yaml).expect("recipe");
        sqlx::query(
            "INSERT INTO recipes_index (id, workspace_id, name, file_path, trigger_type)
             VALUES ('hitl-recipe', ?, 'Receita HITL', '/tmp/hitl.yaml', 'MANUAL')",
        )
        .bind(&ctx.workspace_id)
        .execute(&ctx.state.db)
        .await
        .expect("index");
        let run = store::create_run(
            &ctx.state.db,
            &recipe,
            &ctx.workspace_id,
            None,
            &json!({}),
            "/tmp/out",
            false,
        )
        .await
        .expect("run");
        (ctx, run)
    }

    #[test]
    fn parses_every_reply_shape() {
        assert_eq!(parse_reply("BINARY_GATE", &[], " Aprovar ").0, "APPROVED");
        assert_eq!(parse_reply("BINARY_GATE", &[], "sim").0, "APPROVED");
        assert_eq!(parse_reply("BINARY_GATE", &[], "👍").0, "APPROVED");
        let (kind, payload) = parse_reply("BINARY_GATE", &[], "não");
        assert_eq!(kind, "REJECTED");
        assert_eq!(payload.as_deref(), Some("não"));
        assert_eq!(parse_reply("BINARY_GATE", &[], "> citado\n").0, "FEEDBACK");

        let (kind, _payload) = parse_reply("SINGLE_CHOICE", &["sim".into(), "não".into()], "2");
        assert_eq!(kind, "CHOICE");
        assert_eq!(parse_reply("TEXT", &[], "revisar o texto").0, "FEEDBACK");
        let (kind, payload) = parse_reply("MULTI_CHOICE", &["sim".into(), "não".into()], "2");
        assert_eq!(kind, "CHOICE");
        assert!(payload.expect("payload").contains("\"option_index\":1"));
        assert_eq!(
            parse_reply("MULTI_CHOICE", &["sim".into()], "9").0,
            "FEEDBACK"
        );

        let (kind, payload) = parse_reply("BINARY_GATE", &[], "revisar o texto");
        assert_eq!(kind, "FEEDBACK");
        assert_eq!(payload.as_deref(), Some("revisar o texto"));
    }

    #[test]
    fn extracts_tokens_from_messages() {
        assert_eq!(
            extract_token("responda OD-#AB12CD34").as_deref(),
            Some("AB12CD34")
        );
        assert_eq!(extract_token("#ab12 ok").as_deref(), Some("AB12"));
        assert_eq!(extract_token("sem token"), None);
        assert_eq!(extract_token("OD-#ZZ"), None);
    }

    #[test]
    fn parses_resolution_payloads() {
        assert!(matches!(
            parse_resolution("REJECTED"),
            HitlResolution::Rejected { .. }
        ));
        assert!(matches!(
            parse_resolution("desconhecido"),
            HitlResolution::Approved
        ));
        match parse_resolution(r#"{"path":"out/a.md"}"#) {
            HitlResolution::ArtifactEdited { path } => assert_eq!(path, "out/a.md"),
            other => panic!("unexpected {other:?}"),
        }
        match parse_resolution(r#"{"feedback_text":"ok","attached_files":["a.md"]}"#) {
            HitlResolution::FeedbackProvided {
                feedback_text,
                attached_files,
            } => {
                assert_eq!(feedback_text, "ok");
                assert_eq!(attached_files, vec!["a.md"]);
            }
            other => panic!("unexpected {other:?}"),
        }
        match parse_resolution(r#"{"option_index":1,"option_value":"sim"}"#) {
            HitlResolution::ChoiceSelected {
                option_index,
                option_value,
            } => {
                assert_eq!(option_index, 1);
                assert_eq!(option_value, "sim");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn request_creates_and_dispatches_channels() {
        let (ctx, run) = fixture().await;
        let token = request(
            &ctx.state,
            &run,
            &checkpoint(&["DESKTOP_UI", "UNKNOWN"]),
            "Aprovar envio",
            "Envie o relatório?",
            Some("Publicar"),
            &[],
        )
        .await
        .expect("request");
        assert_eq!(token.len(), 4);
        let stored = store::get_hitl(&ctx.state.db, &token).await.expect("hitl");
        assert_eq!(stored.status, "PENDING");

        resolve(&ctx.state, &token, "DESKTOP_UI", None, "APPROVED", None)
            .await
            .expect("resolve");
        let outcome = wait(&ctx.state, &token, 1).await.expect("wait");
        assert!(matches!(outcome, HitlOutcome::Resolved(_)));

        assert!(
            resolve(&ctx.state, "missing", "DESKTOP_UI", None, "APPROVED", None)
                .await
                .expect("missing")
                .is_none()
        );

        sqlx::query("UPDATE hitl_requests SET status = 'EXPIRED' WHERE token = ?")
            .bind(&token)
            .execute(&ctx.state.db)
            .await
            .expect("expire");
        assert!(matches!(
            wait(&ctx.state, &token, 1).await.expect("expired"),
            HitlOutcome::Expired
        ));
    }

    #[tokio::test]
    async fn email_and_signal_dispatch_explain_missing_configuration() {
        let (ctx, run) = fixture().await;
        let payload = HitlRequestPayload {
            token: "AB12".into(),
            run_id: run.id.clone(),
            workspace_id: ctx.workspace_id.clone(),
            interaction_type: "BINARY_GATE".into(),
            title: "Aprovar".into(),
            message_body: "Seguir?".into(),
            options: vec![],
            attachments: vec![],
        };
        let email = email::dispatch(&ctx.state, &payload)
            .await
            .expect_err("email sem destinatário");
        assert!(format!("{email}").contains("e-mail padrão"));
        let signal = signal::dispatch(&ctx.state, &payload)
            .await
            .expect_err("signal desligado");
        assert!(format!("{signal}").contains("Signal"));
    }
}
