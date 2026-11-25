use crate::{
    app_state::AppState,
    error::AppResult,
    voice::{self, Transcription, VoiceSettings},
};
use axum::{
    Json,
    extract::State,
    http::header,
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
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
    Ok(Json(
        voice::transcribe(&state.db, &state.paths, &body.audio_base64, &body.mime).await?,
    ))
}

pub async fn stt(
    State(state): State<Arc<AppState>>,
    Json(body): Json<TranscribeBody>,
) -> AppResult<Json<Transcription>> {
    transcribe(State(state), Json(body)).await
}

#[derive(Debug, Deserialize)]
pub struct SpeakBody {
    pub text: String,
}

pub async fn speak(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SpeakBody>,
) -> AppResult<Json<Value>> {
    let speech = voice::speak(&state.db, &state.paths, &body.text).await?;
    Ok(Json(json!({
        "path": "",
        "mime": speech.mime,
        "provider": speech.provider,
        "data": STANDARD.encode(speech.bytes),
    })))
}

pub async fn tts(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SpeakBody>,
) -> AppResult<Response> {
    let speech = voice::speak(&state.db, &state.paths, &body.text).await?;
    Ok((
        [
            (header::CONTENT_TYPE, speech.mime),
            (
                header::HeaderName::from_static("x-voice-provider"),
                speech.provider,
            ),
        ],
        speech.bytes,
    )
        .into_response())
}
