use crate::{
    app_state::AppState,
    db::{tasks, workspaces},
    error::{AppError, AppResult},
    events::OpenDeskWsEvent,
};
use axum::{Json, extract::State};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct CaptureBody {
    pub workspace_id: Option<String>,
    pub content: String,
    pub image_base64: Option<String>,
    pub filename: Option<String>,
    pub assigned_agent: Option<String>,
}

/// Quick Capture entry point: text, links and clipboard images become a
/// backlog task without opening the full board.
pub async fn capture(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CaptureBody>,
) -> AppResult<Json<tasks::Task>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    let content = body.content.trim();
    if content.is_empty() && body.image_base64.is_none() {
        return Err(AppError::BadRequest(
            "cole um texto, link ou imagem para capturar".into(),
        ));
    }

    let mut description = content.to_string();
    if let Some(image) = body.image_base64.as_deref() {
        let extension = body
            .filename
            .as_deref()
            .and_then(|name| name.rsplit('.').next())
            .filter(|value| value.len() <= 5)
            .unwrap_or("png");
        let filename = format!(
            "capture-{}.{}",
            chrono::Utc::now().format("%Y%m%d-%H%M%S"),
            extension
        );
        let saved = crate::api::attachments::save_image_base64(
            std::path::Path::new(&workspace.root_path),
            "input/captures",
            &filename,
            image,
        )?;
        if !description.is_empty() {
            description.push_str("\n\n");
        }
        description.push_str(&format!("Arquivo anexado: {}", saved.path));
    }

    let title = content
        .lines()
        .next()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.chars().take(80).collect::<String>())
        .unwrap_or_else(|| {
            format!(
                "Captura de tela {}",
                chrono::Utc::now().format("%d/%m %H:%M")
            )
        });
    let agent = body
        .assigned_agent
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let mut task = tasks::create(
        &state.db,
        &workspace_id,
        tasks::CreateTaskInput {
            title,
            description,
            execution_mode: Some("BUILD_DIRECT".into()),
            priority: Some(2),
            assigned_agent: agent.clone(),
            preferred_model: None,
            preferred_harness: None,
            effort_level: None,
            care_level: Some("standard".into()),
            notify_email: None,
            export_to_gdocs: false,
            tags: vec!["quick-capture".into()],
            origin: Some("CAPTURE".into()),
            parent_task_id: None,
            dedupe: None,
        },
    )
    .await?;
    if agent.is_some() {
        task = tasks::update(
            &state.db,
            &task.id,
            tasks::UpdateTaskInput {
                agent_override: Some(true),
                ..Default::default()
            },
        )
        .await?;
    }
    state.events.publish(OpenDeskWsEvent::TaskUpdated {
        task_id: task.id.clone(),
        workspace_id: task.workspace_id.clone(),
        stage: task.status.clone(),
        phase_status: task.phase_status.clone(),
    });
    Ok(Json(task))
}
