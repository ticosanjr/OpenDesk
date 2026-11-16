use crate::{
    app_state::AppState,
    db::workspaces,
    error::AppResult,
    voice::{self, Speech, Transcription, VoiceSettings},
};
use axum::{Json, extract::State};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

pub async fn status(State(state): State<Arc<AppState>>) -> AppResult<Json<Value>> {
    let settings = voice::get(&state.db).await?;
    let capabilities = voice::capabilities(&settings);
    Ok(Json(json!({
        "settings": settings,
        "capabilities": capabilities,
    })))
}

pub async fn put_settings(
    State(state): State<Arc<AppState>>,
    Json(settings): Json<VoiceSettings>,
) -> AppResult<Json<Value>> {
    let saved = voice::set(&state.db, settings).await?;
    let capabilities = voice::capabilities(&saved);
    Ok(Json(
        json!({ "settings": saved, "capabilities": capabilities }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct TranscribeBody {
    pub audio_base64: String,
    #[serde(default = "default_mime")]
    pub mime: String,
}

fn default_mime() -> String {
    "audio/wav".into()
}

pub async fn transcribe(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TranscribeBody>,
) -> AppResult<Json<Transcription>> {
    let extension = body.mime.rsplit('/').next().unwrap_or("wav");
    Ok(Json(
        voice::transcribe(&state.db, &state.paths, &body.audio_base64, extension).await?,
    ))
}

#[derive(Debug, Deserialize)]
pub struct SpeakBody {
    pub text: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
}

pub async fn speak(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SpeakBody>,
) -> AppResult<Json<Speech>> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, body.workspace_id.as_deref()).await?;
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    Ok(Json(
        voice::speak(
            &state.db,
            &state.paths,
            std::path::Path::new(&workspace.root_path),
            &body.text,
        )
        .await?,
    ))
}
