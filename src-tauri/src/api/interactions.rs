use crate::{
    app_state::AppState,
    db::{
        interactions::{self, CreateInteractionInput, TaskInteraction},
        tasks,
    },
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
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
            options: body.options,
            multiple: body.multiple,
            allow_custom: body.allow_custom,
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
    Ok(Json(interaction))
}

pub async fn resolve(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ResolveInteractionBody>,
) -> AppResult<Json<TaskInteraction>> {
    let current = interactions::get(&state.db, &id).await?;
    if current.status != "PENDING" {
        return Err(AppError::Conflict(
            "esta interação já foi respondida ou expirou".into(),
        ));
    }
    let text = body
        .text
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if body.selected.is_empty() && text.is_none() {
        return Err(AppError::BadRequest(
            "selecione uma opção ou escreva uma resposta".into(),
        ));
    }
    let answer: Value = json!({
        "selected": body.selected,
        "text": text,
    });
    let interaction = interactions::resolve(&state.db, &id, &answer, "RESOLVED").await?;
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
    Ok(Json(interaction))
}
