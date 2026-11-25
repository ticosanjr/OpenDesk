//! OpenAI-compatible HTTP providers for TTS and STT.

use crate::{
    error::{AppError, AppResult},
    paths::AppPaths,
    secrets::SecretStore,
};
use serde_json::json;
use std::time::Duration;

use super::VoiceSettings;
use super::engines::AudioOut;

const TIMEOUT: Duration = Duration::from_secs(60);

fn endpoint_url(base: &str, path: &str) -> String {
    let base = base.trim().trim_end_matches('/');
    if base.ends_with("/v1") {
        format!("{base}/{path}")
    } else {
        format!("{base}/v1/{path}")
    }
}

fn secret(paths: &AppPaths, reference: &str) -> AppResult<String> {
    SecretStore::new(paths)
        .get("voice", reference)?
        .or(SecretStore::new(paths).get("integrations", reference)?)
        .ok_or_else(|| {
            AppError::Unavailable(
                format!("segredo de voz '{reference}' não encontrado"),
                "Salve a chave em Configurações → Integrações ou use OPENDESK_VOICE_<REF>.".into(),
            )
        })
}

pub(super) async fn endpoint_speech(
    settings: &VoiceSettings,
    paths: &AppPaths,
    text: &str,
) -> AppResult<AudioOut> {
    let token = secret(paths, &settings.tts_secret_ref)?;
    let url = endpoint_url(&settings.tts_endpoint, "audio/speech");
    let model = if settings.tts_model.trim().is_empty() {
        "tts-1".to_string()
    } else {
        settings.tts_model.trim().to_string()
    };
    let voice = if settings.voice.trim().is_empty() {
        "alloy".to_string()
    } else {
        settings.voice.trim().to_string()
    };
    let response = reqwest::Client::new()
        .post(&url)
        .bearer_auth(token)
        .json(&json!({
            "model": model,
            "input": text,
            "voice": voice,
            "response_format": "wav",
        }))
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|error| AppError::Daemon(format!("TTS endpoint: {error}")))?;
    if !response.status().is_success() {
        return Err(AppError::Daemon(format!(
            "TTS endpoint respondeu {}",
            response.status()
        )));
    }
    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("audio/wav")
        .to_string();
    let bytes = response
        .bytes()
        .await
        .map_err(|error| AppError::Daemon(format!("TTS endpoint: {error}")))?
        .to_vec();
    if bytes.is_empty() {
        return Err(AppError::Daemon("TTS endpoint devolveu áudio vazio".into()));
    }
    Ok(AudioOut {
        bytes,
        mime,
        provider: "openai-compatible".into(),
    })
}

pub(super) async fn endpoint_transcribe(
    settings: &VoiceSettings,
    paths: &AppPaths,
    bytes: &[u8],
    mime: &str,
) -> AppResult<(String, String)> {
    let token = secret(paths, &settings.stt_secret_ref)?;
    let url = endpoint_url(&settings.stt_endpoint, "audio/transcriptions");
    let extension = match mime.rsplit('/').next().unwrap_or("wav") {
        "mpeg" | "mp3" => "mp3",
        "ogg" => "ogg",
        "webm" => "webm",
        "mp4" | "m4a" => "m4a",
        _ => "wav",
    };
    let part = reqwest::multipart::Part::bytes(bytes.to_vec())
        .file_name(format!("audio.{extension}"))
        .mime_str(mime)
        .map_err(|error| AppError::Internal(error.to_string()))?;
    let mut form = reqwest::multipart::Form::new().part("file", part).text(
        "model",
        if settings.stt_model.trim().is_empty() {
            "whisper-1".to_string()
        } else {
            settings.stt_model.trim().to_string()
        },
    );
    if !settings.language.trim().is_empty() {
        form = form.text("language", settings.language.trim().to_string());
    }
    let response = reqwest::Client::new()
        .post(&url)
        .bearer_auth(token)
        .multipart(form)
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|error| AppError::Daemon(format!("STT endpoint: {error}")))?;
    if !response.status().is_success() {
        return Err(AppError::Daemon(format!(
            "STT endpoint respondeu {}",
            response.status()
        )));
    }
    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|error| AppError::Daemon(format!("STT endpoint: {error}")))?;
    let text = body
        .get("text")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    if text.is_empty() {
        return Err(AppError::Daemon(
            "STT endpoint não retornou transcrição".into(),
        ));
    }
    Ok((text, "openai-compatible".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_url_keeps_a_single_v1() {
        assert_eq!(
            endpoint_url("http://127.0.0.1:8080", "audio/speech"),
            "http://127.0.0.1:8080/v1/audio/speech"
        );
        assert_eq!(
            endpoint_url("http://127.0.0.1:8080/v1/", "audio/speech"),
            "http://127.0.0.1:8080/v1/audio/speech"
        );
    }
}
