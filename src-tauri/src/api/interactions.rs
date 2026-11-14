use crate::{
    app_state::AppState,
    db::{
        interactions::{self, CreateInteractionInput, TaskInteraction},
        tasks,
    },
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
    hitl::HitlKind,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct InteractionQuery {
    pub workspace_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct AskInteractionBody {
    pub question: String,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub options: Vec<String>,
    /// HITL 2.0 structure: APPROVAL, SINGLE_CHOICE, MULTI_CHOICE or TEXT.
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub multiple: bool,
    #[serde(default = "default_true")]
    pub allow_custom: bool,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub timeout_minutes: Option<i64>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Default)]
pub struct ResolveInteractionBody {
    #[serde(default)]
    pub selected: Vec<String>,
    #[serde(default)]
    pub text: Option<String>,
    /// `approve`, `deny` or `answer`. Older clients omit it and answer.
    #[serde(default)]
    pub decision: Option<String>,
}

pub async fn list_pending(
    State(state): State<Arc<AppState>>,
    Query(query): Query<InteractionQuery>,
) -> AppResult<Json<Vec<TaskInteraction>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        interactions::pending_for_workspace(&state.db, &workspace_id).await?,
    ))
}

pub async fn list_for_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<TaskInteraction>>> {
    tasks::get(&state.db, &id).await?;
    Ok(Json(interactions::pending_for_task(&state.db, &id).await?))
}

pub async fn get_interaction(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<TaskInteraction>> {
    Ok(Json(interactions::get(&state.db, &id).await?))
}

pub async fn ask(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<AskInteractionBody>,
) -> AppResult<Json<TaskInteraction>> {
    let question = body.question.trim();
    if question.is_empty() {
        return Err(AppError::BadRequest(
            "o agente precisa enviar uma pergunta".into(),
        ));
    }
    let kind = normalize_kind(body.kind.as_deref(), &body.options, body.multiple)?;
    let options = if kind.requires_options() {
        body.options
    } else {
        Vec::new()
    };
    if kind.requires_options() && options.is_empty() {
        return Err(AppError::BadRequest(format!(
            "{} exige ao menos uma opção",
            kind.as_str()
        )));
    }
    let task = tasks::get(&state.db, &id).await?;
    let minutes = body.timeout_minutes.unwrap_or(10).clamp(1, 120);
    let expires_at = (Utc::now() + Duration::minutes(minutes)).to_rfc3339();
    let interaction = interactions::create(
        &state.db,
        CreateInteractionInput {
            workspace_id: task.workspace_id.clone(),
            task_id: task.id.clone(),
            run_id: body.run_id,
            question: question.to_string(),
            context_markdown: body.context,
            options,
            kind: kind.as_str().to_string(),
            multiple: kind == HitlKind::MultiChoice,
            allow_custom: body.allow_custom || kind == HitlKind::Text,
            expires_at,
        },
    )
    .await?;
    tasks::replace_phase_status(
        &state.db,
        &task.id,
        Some("awaiting_input"),
        Some("IN_REVIEW"),
    )
    .await?;
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: "IN_REVIEW".into(),
        phase_status: Some("awaiting_input".into()),
    });
    let notify_state = state.clone();
    let notify_interaction = interaction.clone();
    let notify_task = task.clone();
    tokio::spawn(async move {
        if let Err(error) = crate::integrations::telegram::notify_task_interaction(
            &notify_state,
            &notify_interaction,
            &notify_task,
        )
        .await
        {
            tracing::warn!(
                error = %error,
                interaction = %notify_interaction.id,
                "Telegram HITL notification failed"
            );
        }
        if let Err(error) = crate::integrations::signal::notify_task_interaction(
            &notify_state,
            &notify_interaction,
            &notify_task,
        )
        .await
        {
            tracing::warn!(
                error = %error,
                interaction = %notify_interaction.id,
                "Signal HITL notification failed"
            );
        }
    });
    Ok(Json(interaction))
}

pub async fn resolve(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ResolveInteractionBody>,
) -> AppResult<Json<TaskInteraction>> {
    Ok(Json(
        resolve_internal(&state, &id, body.selected, body.text, body.decision).await?,
    ))
}

fn normalize_kind(value: Option<&str>, options: &[String], multiple: bool) -> AppResult<HitlKind> {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => HitlKind::parse(value)
            .ok_or_else(|| AppError::BadRequest(format!("tipo de interação inválido: {value}"))),
        None => Ok(HitlKind::from_options(options, multiple)),
    }
}

fn normalize_decision(value: Option<&str>) -> &'static str {
    match value.map(str::trim).map(str::to_lowercase).as_deref() {
        Some("approve") | Some("aprovar") | Some("aprovado") => "approve",
        Some("deny") | Some("negar") | Some("rejected") | Some("rejeitar") => "deny",
        _ => "answer",
    }
}

/// Shared by the HTTP route, Telegram and Signal. Keeps validation and the task
/// phase transition in one place.
pub(crate) async fn resolve_internal(
    state: &Arc<AppState>,
    id: &str,
    selected: Vec<String>,
    text: Option<String>,
    decision: Option<String>,
) -> AppResult<TaskInteraction> {
    let current = interactions::get(&state.db, id).await?;
    if current.status != "PENDING" {
        return Err(AppError::Conflict(
            "esta interação já foi respondida ou expirou".into(),
        ));
    }
    let current_options =
        serde_json::from_str::<Vec<String>>(&current.options_json).unwrap_or_default();
    let kind = HitlKind::effective(&current.kind, &current_options, current.multiple);
    let decision = normalize_decision(decision.as_deref());
    let text = text
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if decision == "answer" {
        match kind {
            HitlKind::Approval => {
                return Err(AppError::BadRequest(
                    "aprovação exige aprovar ou negar".into(),
                ));
            }
            HitlKind::SingleChoice => {
                if selected.len() != 1 && text.is_none() {
                    return Err(AppError::BadRequest(
                        "escolha exatamente uma opção ou escreva um payload".into(),
                    ));
                }
            }
            HitlKind::MultiChoice => {
                if selected.is_empty() && text.is_none() {
                    return Err(AppError::BadRequest(
                        "selecione ao menos uma opção ou escreva um payload".into(),
                    ));
                }
            }
            HitlKind::Text => {
                if text.is_none() {
                    return Err(AppError::BadRequest(
                        "escreva uma resposta para continuar".into(),
                    ));
                }
            }
        }
    }
    if kind != HitlKind::MultiChoice && selected.len() > 1 {
        return Err(AppError::BadRequest(
            "esta interação aceita apenas uma opção".into(),
        ));
    }
    let answer: Value = json!({
        "decision": decision,
        "selected": selected,
        "text": text,
    });
    let interaction = interactions::resolve(&state.db, id, &answer, "RESOLVED").await?;
    let task = tasks::get(&state.db, &interaction.task_id).await?;
    if !matches!(task.status.as_str(), "DONE" | "CANCELLED" | "BLOCKED") {
        tasks::replace_phase_status(&state.db, &task.id, Some("running"), Some("IN_PROGRESS"))
            .await?;
        state.events.publish(OpenDeskWsEvent::TaskUpdated {
            task_id: task.id.clone(),
            workspace_id: task.workspace_id.clone(),
            stage: "IN_PROGRESS".into(),
            phase_status: Some("running".into()),
        });
    }
    Ok(interaction)
}

pub(crate) fn parse_option_indices(
    answer: &str,
    options: &[String],
    multiple: bool,
) -> Vec<String> {
    if options.is_empty() {
        return Vec::new();
    }
    let mut selected = Vec::new();
    for part in answer
        .split([',', ' '])
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let Ok(index) = part.parse::<usize>() else {
            return Vec::new();
        };
        if index == 0 || index > options.len() {
            return Vec::new();
        }
        selected.push(options[index - 1].clone());
        if !multiple {
            break;
        }
    }
    selected
}

/// Interprets a remote text reply for any HITL kind: `aprovar [payload]`,
/// `negar [payload]`, option numbers, or free text.
pub(crate) fn parse_remote_answer(
    interaction: &TaskInteraction,
    answer: &str,
) -> (String, Vec<String>, Option<String>) {
    let options =
        serde_json::from_str::<Vec<String>>(&interaction.options_json).unwrap_or_default();
    let kind = HitlKind::parse(&interaction.kind).unwrap_or(HitlKind::Text);
    let trimmed = answer.trim();
    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let first = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or_default().trim();
    let payload = (!rest.is_empty()).then(|| rest.to_string());
    match first.to_lowercase().as_str() {
        "aprovar" | "aprovo" | "approve" | "sim" | "ok" => ("approve".into(), Vec::new(), payload),
        "negar" | "rejeitar" | "reject" | "não" | "nao" | "no" => {
            ("deny".into(), Vec::new(), payload)
        }
        _ if kind.requires_options() => {
            // Consume leading option numbers (`1`, `1,2`, `1, 2`) and keep the
            // remaining words as the optional payload.
            let mut numbers = Vec::new();
            let mut rest_words = Vec::new();
            let mut in_numbers = true;
            for token in trimmed.split_whitespace() {
                if in_numbers
                    && token
                        .split(',')
                        .all(|part| part.is_empty() || part.parse::<usize>().is_ok())
                {
                    numbers.push(token);
                } else {
                    in_numbers = false;
                    rest_words.push(token);
                }
            }
            let selected =
                parse_option_indices(&numbers.join(" "), &options, kind == HitlKind::MultiChoice);
            if selected.is_empty() {
                (
                    "answer".into(),
                    Vec::new(),
                    (!trimmed.is_empty()).then(|| trimmed.to_string()),
                )
            } else {
                let payload = rest_words.join(" ");
                (
                    "answer".into(),
                    selected,
                    (!payload.is_empty()).then_some(payload),
                )
            }
        }
        _ => (
            "answer".into(),
            Vec::new(),
            (!trimmed.is_empty()).then(|| trimmed.to_string()),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interaction(kind: &str, options: &str) -> TaskInteraction {
        TaskInteraction {
            id: "abcdef12-0000-0000-0000-000000000000".into(),
            workspace_id: "ws".into(),
            task_id: "task".into(),
            run_id: None,
            question: "Pergunta".into(),
            context_markdown: None,
            options_json: options.into(),
            kind: kind.into(),
            multiple: kind == "MULTI_CHOICE",
            allow_custom: true,
            status: "PENDING".into(),
            answer_json: None,
            expires_at: "2999-01-01T00:00:00Z".into(),
            created_at: String::new(),
            resolved_at: None,
        }
    }

    #[test]
    fn parses_remote_answers_for_every_kind() {
        let approval = interaction("APPROVAL", "[]");
        assert_eq!(
            parse_remote_answer(&approval, "aprovar com ajuste"),
            ("approve".into(), Vec::new(), Some("com ajuste".into()))
        );
        assert_eq!(
            parse_remote_answer(&approval, "negar"),
            ("deny".into(), Vec::new(), None)
        );

        let choice = interaction("MULTI_CHOICE", r#"["A","B"]"#);
        assert_eq!(
            parse_remote_answer(&choice, "1,2 foco no B"),
            (
                "answer".into(),
                vec!["A".to_string(), "B".to_string()],
                Some("foco no B".into())
            )
        );

        let text = interaction("TEXT", "[]");
        assert_eq!(
            parse_remote_answer(&text, "sexta-feira"),
            ("answer".into(), Vec::new(), Some("sexta-feira".into()))
        );
    }
}
