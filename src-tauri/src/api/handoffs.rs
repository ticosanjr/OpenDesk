use crate::{
    app_state::AppState,
    db::{
        budget,
        handoffs::{self, CreateHandoffInput, Handoff},
        tasks,
    },
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Debug, Deserialize, Default)]
pub struct HandoffQuery {
    pub workspace_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
}

pub async fn list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HandoffQuery>,
) -> AppResult<Json<Vec<Handoff>>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    Ok(Json(
        handoffs::list(
            &state.db,
            &workspace_id,
            query.status.as_deref(),
            query.limit.unwrap_or(100),
        )
        .await?,
    ))
}

pub async fn get(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Handoff>> {
    Ok(Json(handoffs::get(&state.db, &id).await?))
}

pub async fn create(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HandoffQuery>,
    Json(input): Json<CreateHandoffInput>,
) -> AppResult<Json<Handoff>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let handoff = handoffs::create(&state.db, &workspace_id, input).await?;
    publish(&state, &handoff);
    Ok(Json(handoff))
}

#[derive(Debug, Deserialize, Default)]
pub struct AcceptBody {
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub note: Option<String>,
}

pub async fn accept(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<AcceptBody>,
) -> AppResult<Json<Value>> {
    let handoff = handoffs::get(&state.db, &id).await?;
    if handoff.status != "PENDING" {
        return Err(AppError::Conflict(format!(
            "handoff já está {}",
            handoff.status
        )));
    }

    if let Some(sub_budget) = handoff.sub_budget_usd
        && !body.force
    {
        let status = budget::status(&state.db).await?;
        let daily =
            status.daily_limit > 0.0 && status.daily_spent + sub_budget > status.daily_limit;
        let monthly =
            status.monthly_limit > 0.0 && status.monthly_spent + sub_budget > status.monthly_limit;
        if status.hard_stop || daily || monthly {
            return Err(AppError::Conflict(format!(
                "aceitar este handoff (US$ {sub_budget:.2}) excede o orçamento disponível; use force=true para confirmar"
            )));
        }
    }

    let description = render_description(&handoff, body.note.as_deref());
    let task = tasks::create(
        &state.db,
        &handoff.workspace_id,
        tasks::CreateTaskInput {
            title: format!(
                "[handoff] {}",
                handoff.objective.chars().take(70).collect::<String>()
            ),
            description,
            execution_mode: Some("BUILD_DIRECT".into()),
            priority: Some(2),
            assigned_agent: Some(handoff.target_agent.clone()),
            preferred_model: None,
            preferred_harness: None,
            effort_level: None,
            care_level: Some("standard".into()),
            notify_email: None,
            export_to_gdocs: false,
            tags: vec![
                format!("handoff:{}", handoff.id),
                format!("from:{}", handoff.source_agent),
            ],
            origin: Some("DELEGATION".into()),
            parent_task_id: handoff.task_id.clone(),
            dedupe: Some(false),
        },
    )
    .await?;

    handoffs::transition(&state.db, &id, "ACCEPTED", None).await?;
    let accepted = handoffs::merge_result(
        &state.db,
        &id,
        json!({
            "task_id": task.id,
            "accepted_by": "human",
            "note": body.note,
            "forced": body.force,
        }),
    )
    .await?;
    publish(&state, &accepted);
    Ok(Json(json!({ "handoff": accepted, "task": task })))
}

#[derive(Debug, Deserialize)]
pub struct RejectBody {
    #[serde(default)]
    pub reason: Option<String>,
}

pub async fn reject(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<RejectBody>,
) -> AppResult<Json<Handoff>> {
    let handoff = handoffs::transition(
        &state.db,
        &id,
        "REJECTED",
        Some(json!({ "reason": body.reason })),
    )
    .await?;
    publish(&state, &handoff);
    Ok(Json(handoff))
}

pub async fn cancel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<Handoff>> {
    let handoff = handoffs::transition(&state.db, &id, "CANCELLED", None).await?;
    publish(&state, &handoff);
    Ok(Json(handoff))
}

#[derive(Debug, Deserialize)]
pub struct FinishBody {
    pub status: String,
    #[serde(default)]
    pub result: Value,
}

pub async fn finish(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<FinishBody>,
) -> AppResult<Json<Handoff>> {
    let status = body.status.to_uppercase();
    if !matches!(status.as_str(), "DONE" | "FAILED") {
        return Err(AppError::BadRequest(
            "finish aceita apenas DONE ou FAILED".into(),
        ));
    }
    let handoff = handoffs::transition(&state.db, &id, &status, Some(body.result)).await?;
    publish(&state, &handoff);
    Ok(Json(handoff))
}

fn render_description(handoff: &Handoff, note: Option<&str>) -> String {
    let artifacts = handoff.input_artifacts();
    let acceptance = handoff.acceptance();
    let schema = handoff
        .output_schema_json
        .parse::<Value>()
        .map(|value| serde_json::to_string_pretty(&value).unwrap_or_default())
        .unwrap_or_default();
    let mut text = format!(
        "# Handoff {}\n\n## Objetivo\n{}\n\n## Origem → destino\n{} → {}\n",
        handoff.id, handoff.objective, handoff.source_agent, handoff.target_agent
    );
    if !artifacts.is_empty() {
        text.push_str("\n## Artefatos de entrada\n");
        for artifact in artifacts {
            text.push_str(&format!("- {artifact}\n"));
        }
    }
    if !acceptance.is_empty() {
        text.push_str("\n## Critérios de aceite\n");
        for item in acceptance {
            text.push_str(&format!("- {item}\n"));
        }
    }
    if !schema.is_empty() {
        text.push_str(&format!("\n## Schema de saída\n```json\n{schema}\n```\n"));
    }
    if let Some(note) = note.map(str::trim).filter(|value| !value.is_empty()) {
        text.push_str(&format!("\n## Observação da aprovação\n{note}\n"));
    }
    text
}

fn publish(state: &Arc<AppState>, handoff: &Handoff) {
    state.events.publish(OpenDeskWsEvent::HandoffUpdated {
        handoff_id: handoff.id.clone(),
        workspace_id: handoff.workspace_id.clone(),
        status: handoff.status.clone(),
        target_agent: handoff.target_agent.clone(),
    });
}
