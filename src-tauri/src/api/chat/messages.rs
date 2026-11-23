use crate::{
    app_state::AppState,
    chat,
    db::{chat as chat_db, workspaces},
    dsh_router::{context_packer::PromptSection, prompt_templates::PromptTemplates},
    error::{AppError, AppResult},
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct AttachmentBody {
    pub filename: String,
    pub base64: String,
}

#[derive(Debug, Deserialize)]
pub struct MessageBody {
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<AttachmentBody>,
}

pub async fn send_message(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<MessageBody>,
) -> AppResult<Json<chat::ChatReply>> {
    let thread = chat_db::get_thread(&state.db, &id).await?;
    let mut content = body.content;
    if !body.attachments.is_empty() {
        let workspace = crate::db::workspaces::get(&state.db, &thread.workspace_id).await?;
        for attachment in body.attachments {
            let saved = crate::api::attachments::save_base64(
                std::path::Path::new(&workspace.root_path),
                "input/chat",
                &attachment.filename,
                &attachment.base64,
            )?;
            content.push_str(&format!("\n\nArquivo anexado: {}", saved.path));
        }
    }
    let templates = crate::dsh_router::prompt_templates::PromptTemplates::load(
        &state.db,
        Some(&thread.workspace_id),
    )
    .await
    .unwrap_or_default();
    let context =
        chat::channel_context_with(&templates, "Desktop", &thread.workspace_id, &thread.id);
    // Detach the turn so a browser reload does not abort the OpenCode work. The
    // handler still waits for the reply in the normal case, while the task keeps
    // running (and persisting messages) if the request is dropped.
    let handle = tokio::spawn(async move {
        chat::send_message_with_context(&state, thread, &content, Some(&context)).await
    });
    match handle.await {
        Ok(result) => Ok(Json(result?)),
        Err(error) => Err(AppError::Internal(format!(
            "chat task terminated unexpectedly: {error}"
        ))),
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct PromptPreviewQuery {
    pub content: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChatPromptPreview {
    pub thread: chat_db::ChatThread,
    pub prompt: String,
    pub sections: Vec<PromptSection>,
    pub opencode: crate::api::prompt_layer::OpenCodeLayer,
    pub composed: String,
}

/// Preview the exact payload the next chat message will send, plus the
/// OpenCode layer readable from native files.
pub async fn prompt_preview(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<PromptPreviewQuery>,
) -> AppResult<Json<ChatPromptPreview>> {
    let thread = chat_db::get_thread(&state.db, &id).await?;
    let workspace = workspaces::get(&state.db, &thread.workspace_id).await?;
    let templates = PromptTemplates::load(&state.db, Some(&thread.workspace_id)).await?;
    let personal_body =
        crate::personal_context::render_body(&state.db, &thread.workspace_id).await?;
    let personal = if personal_body.is_empty() {
        String::new()
    } else {
        crate::dsh_router::prompt_templates::render(
            &templates.personal_block,
            &[("personal_context", &personal_body)],
        )
    };
    let channel = chat::channel_context_with(
        &templates,
        &cap_first(&thread.source),
        &thread.workspace_id,
        &thread.id,
    );
    let content = match query.content.map(|value| value.trim().to_string()) {
        Some(value) if !value.is_empty() => value,
        _ => chat_db::list_messages(&state.db, &id)
            .await?
            .into_iter()
            .rev()
            .find(|message| message.role == "user")
            .map(|message| message.content)
            .unwrap_or_default(),
    };
    let mut sections = Vec::new();
    if !personal.trim().is_empty() {
        sections.push(PromptSection {
            id: "personal".into(),
            label: "Contexto pessoal".into(),
            source: "Contexto pessoal".into(),
            content: personal.trim().to_string(),
        });
    }
    sections.push(PromptSection {
        id: "channel".into(),
        label: "Prompt do chat".into(),
        source: cap_first(&thread.source),
        content: channel.clone(),
    });
    let memory_policy = crate::dsh_router::prompt_templates::render(&templates.memory_policy, &[]);
    if !memory_policy.trim().is_empty() {
        sections.push(PromptSection {
            id: "memory_policy".into(),
            label: "Política de memória".into(),
            source: "Aprendizado/MCP".into(),
            content: memory_policy.clone(),
        });
    }
    if !content.trim().is_empty() {
        sections.push(PromptSection {
            id: "message".into(),
            label: "Mensagem do usuário".into(),
            source: "Chat".into(),
            content: content.clone(),
        });
    }
    let rendered = sections
        .iter()
        .filter(|section| section.id != "message")
        .cloned()
        .collect::<Vec<_>>();
    let prompt = crate::dsh_router::context_packer::render_sections_with(
        &crate::dsh_router::context_packer::PromptBlocks {
            harness_id: "opencode",
            untrusted_data: &content,
            output_schema: None,
            untrusted_wrapper: &templates.untrusted_wrapper,
            json_schema_contract: &templates.json_schema_contract,
        },
        &rendered,
    );
    let layer = crate::api::prompt_layer::snapshot(
        std::path::Path::new(&workspace.root_path),
        &thread.agent,
    );
    let composed = crate::api::prompt_layer::compose(&prompt, &layer);
    Ok(Json(ChatPromptPreview {
        thread,
        prompt,
        sections,
        opencode: layer,
        composed,
    }))
}

fn cap_first(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Desktop".into(),
    }
}

/// Stops the active chat turn. The wait loop sees the cancellation token and
/// marks the chat turn as CANCELLED; OpenCode is interrupted as well.
pub async fn interrupt_thread(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let thread = chat_db::get_thread(&state.db, &id).await?;
    let (turn_id, session_id) = match state.cancel_chat_turn(&thread.id) {
        Some(value) => value,
        None => {
            let Some(active) = crate::db::chat_turns::active_for_thread(&state.db, &id).await?
            else {
                return Err(AppError::Conflict(
                    "nenhuma execução em andamento nesta conversa".into(),
                ));
            };
            let _ = crate::db::chat_turns::finish(
                &state.db,
                &active.id,
                "CANCELLED",
                active.assistant_content.as_deref(),
                &crate::db::runs::RunUsage::default(),
                active.duration_ms,
                Some("cancelled"),
                Some("interrompido pelo usuário"),
            )
            .await;
            (active.id, thread.opencode_session_id.clone())
        }
    };
    let session = session_id.or(thread.opencode_session_id);
    if let Some(session) = session
        && let Some(client) = state.daemon.client().await
    {
        let _ = client.interrupt(&session).await;
    }
    Ok(Json(serde_json::json!({ "ok": true, "turn_id": turn_id })))
}
