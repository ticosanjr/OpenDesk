//! Voice input/output through host engines.
//!
//! OpenDesk does not bundle a model: transcription delegates to `whisper-cli`
//! (or an OpenAI-compatible endpoint) and speech to `piper`, `espeak-ng`,
//! an endpoint or a configured command. Commands are templates with `{audio}`,
//! `{out_base}`, `{out_audio}`, `{text}`, `{voice}` and `{language}`.

use crate::{
    db::settings,
    error::{AppError, AppResult},
    paths::AppPaths,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::time::Instant;

#[path = "voice_engines.rs"]
mod engines;
#[path = "voice_http.rs"]
mod voice_http;

pub const VOICE_SETTING: &str = "voice_settings";
const MAX_AUDIO_BYTES: usize = 25 * 1024 * 1024;
const MAX_TEXT: usize = 4_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceSettings {
    pub stt_command: String,
    pub stt_args: Vec<String>,
    pub stt_provider: String,
    pub stt_endpoint: String,
    pub stt_model: String,
    pub stt_secret_ref: String,
    pub tts_command: String,
    pub tts_args: Vec<String>,
    pub tts_provider: String,
    pub tts_endpoint: String,
    pub tts_model: String,
    pub tts_secret_ref: String,
    pub language: String,
    pub voice: String,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        let stt = engines::detect(&["whisper-cli", "whisper"]).unwrap_or_default();
        let tts = engines::detect(&["piper", "espeak-ng", "espeak", "say"]).unwrap_or_default();
        Self {
            stt_command: stt,
            stt_args: vec![
                "-f".into(),
                "{audio}".into(),
                "-l".into(),
                "{language}".into(),
                "-otxt".into(),
                "-of".into(),
                "{out_base}".into(),
                "-nt".into(),
            ],
            stt_provider: "auto".into(),
            stt_endpoint: String::new(),
            stt_model: String::new(),
            stt_secret_ref: String::new(),
            tts_command: tts,
            tts_args: vec!["{text}".into()],
            tts_provider: "auto".into(),
            tts_endpoint: String::new(),
            tts_model: String::new(),
            tts_secret_ref: String::new(),
            language: "pt".into(),
            voice: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VoiceCapabilities {
    pub stt: Option<String>,
    pub tts: Option<String>,
    pub available: bool,
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_hint: Option<String>,
}

fn stt_hint() -> &'static str {
    "Instale whisper.cpp (whisper-cli) e ffmpeg, ou configure um endpoint STT OpenAI-compatible com secret_ref."
}

fn tts_hint() -> &'static str {
    "Instale piper ou espeak-ng (`sudo apt install espeak-ng`), ou configure um endpoint TTS OpenAI-compatible com secret_ref."
}

pub fn capabilities(settings: &VoiceSettings) -> VoiceCapabilities {
    let stt_ready = engines::stt_available(settings);
    let tts_ready = engines::tts_available(settings);
    let stt = if stt_ready {
        if !settings.stt_command.trim().is_empty() {
            Some(settings.stt_command.clone())
        } else if !settings.stt_endpoint.trim().is_empty() {
            Some("openai-compatible".into())
        } else {
            engines::detect(&["whisper-cli", "whisper"])
        }
    } else {
        None
    };
    let tts = if tts_ready {
        if !settings.tts_command.trim().is_empty() {
            Some(settings.tts_command.clone())
        } else if !settings.tts_endpoint.trim().is_empty() {
            Some("openai-compatible".into())
        } else {
            engines::detect(&["piper", "espeak-ng", "espeak", "say"])
        }
    } else {
        None
    };
    let reason = match (stt_ready, tts_ready) {
        (false, false) => Some("instale um motor de fala e um de transcrição".into()),
        (false, true) => Some("nenhum motor de transcrição encontrado".into()),
        (true, false) => Some("nenhum motor de fala encontrado".into()),
        _ => None,
    };
    let install_hint = if !stt_ready && !tts_ready {
        Some(format!("{} {}", stt_hint(), tts_hint()))
    } else if !stt_ready {
        Some(stt_hint().into())
    } else if !tts_ready {
        Some(tts_hint().into())
    } else {
        None
    };
    VoiceCapabilities {
        available: stt_ready && tts_ready,
        stt,
        tts,
        reason,
        install_hint,
    }
}

pub async fn get(pool: &SqlitePool) -> AppResult<VoiceSettings> {
    let value = settings::get(pool, VOICE_SETTING).await?;
    Ok(value
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default())
}

pub async fn set(pool: &SqlitePool, mut value: VoiceSettings) -> AppResult<VoiceSettings> {
    value.stt_command = value.stt_command.trim().to_string();
    value.tts_command = value.tts_command.trim().to_string();
    value.stt_provider = value.stt_provider.trim().to_string();
    value.tts_provider = value.tts_provider.trim().to_string();
    value.stt_endpoint = value.stt_endpoint.trim().to_string();
    value.tts_endpoint = value.tts_endpoint.trim().to_string();
    value.stt_model = value.stt_model.trim().to_string();
    value.tts_model = value.tts_model.trim().to_string();
    value.stt_secret_ref = value.stt_secret_ref.trim().to_string();
    value.tts_secret_ref = value.tts_secret_ref.trim().to_string();
    value.language = value.language.trim().to_string();
    if value.language.is_empty() {
        value.language = "pt".into();
    }
    settings::set(
        pool,
        VOICE_SETTING,
        &serde_json::to_value(&value).map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .await?;
    Ok(value)
}

#[derive(Debug, Clone, Serialize)]
pub struct Transcription {
    pub text: String,
    pub engine: String,
    pub provider: String,
    pub language: String,
    pub duration_ms: u64,
}

pub async fn transcribe(
    pool: &SqlitePool,
    paths: &AppPaths,
    audio_base64: &str,
    mime: &str,
) -> AppResult<Transcription> {
    let settings = get(pool).await?;
    let capabilities = capabilities(&settings);
    if !capabilities.available && capabilities.stt.is_none() {
        return Err(AppError::Unavailable(
            capabilities
                .reason
                .unwrap_or_else(|| "transcrição indisponível".into()),
            capabilities
                .install_hint
                .unwrap_or_else(|| stt_hint().into()),
        ));
    }
    let bytes = STANDARD
        .decode(audio_base64)
        .map_err(|_| AppError::BadRequest("áudio base64 inválido".into()))?;
    if bytes.is_empty() || bytes.len() > MAX_AUDIO_BYTES {
        return Err(AppError::BadRequest("áudio vazio ou acima de 25 MB".into()));
    }
    let started = Instant::now();
    let (text, provider) = engines::transcribe(&settings, paths, &bytes, mime).await?;
    Ok(Transcription {
        text: text.trim().chars().take(MAX_TEXT).collect(),
        engine: provider.clone(),
        provider,
        language: settings.language,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

pub struct Speech {
    pub bytes: Vec<u8>,
    pub mime: String,
    pub provider: String,
}

pub async fn speak(pool: &SqlitePool, paths: &AppPaths, text: &str) -> AppResult<Speech> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > MAX_TEXT {
        return Err(AppError::BadRequest(
            "texto vazio ou acima de 4000 caracteres".into(),
        ));
    }
    let settings = get(pool).await?;
    let capabilities = capabilities(&settings);
    if !capabilities.available && capabilities.tts.is_none() {
        return Err(AppError::Unavailable(
            capabilities
                .reason
                .unwrap_or_else(|| "fala indisponível".into()),
            capabilities
                .install_hint
                .unwrap_or_else(|| tts_hint().into()),
        ));
    }
    let audio = engines::speak(&settings, paths, text).await?;
    Ok(Speech {
        bytes: audio.bytes,
        mime: audio.mime,
        provider: audio.provider,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn persists_settings_and_reports_missing_engines() {
        let ctx = crate::test_support::context().await;
        let saved = set(
            &ctx.state.db,
            VoiceSettings {
                stt_command: "definitely-missing-stt".into(),
                stt_provider: "whisper".into(),
                tts_command: "definitely-missing-tts".into(),
                tts_provider: "piper".into(),
                language: "en".into(),
                ..Default::default()
            },
        )
        .await
        .expect("set");
        assert_eq!(saved.language, "en");
        let loaded = get(&ctx.state.db).await.expect("get");
        assert_eq!(loaded.stt_command, "definitely-missing-stt");
        let capabilities = capabilities(&loaded);
        assert!(!capabilities.available);
        assert!(capabilities.install_hint.is_some());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn custom_tts_command_returns_audio_bytes() {
        use std::os::unix::fs::PermissionsExt;
        let ctx = crate::test_support::context().await;
        let temp = tempfile::TempDir::new().expect("temp");
        let script = temp.path().join("tts.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\nout=\"\"\nwhile [ $# -gt 0 ]; do case \"$1\" in --output_file) out=\"$2\"; shift 2;; *) shift;; esac; done\nprintf 'RIFFtest-audio' > \"$out\"\n",
        )
        .expect("script");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        set(
            &ctx.state.db,
            VoiceSettings {
                tts_command: script.to_string_lossy().to_string(),
                tts_args: vec!["--output_file".into(), "{out_audio}".into()],
                tts_provider: "custom".into(),
                ..Default::default()
            },
        )
        .await
        .expect("settings");
        let speech = speak(&ctx.state.db, &ctx.state.paths, "olá")
            .await
            .expect("speech");
        assert_eq!(speech.provider, "custom");
        assert!(speech.bytes.starts_with(b"RIFF"));
    }

    #[test]
    fn capabilities_accepts_existing_commands() {
        let temp = tempfile::TempDir::new().expect("temp");
        let stt = temp.path().join("whisper-cli");
        let tts = temp.path().join("espeak-ng");
        std::fs::write(&stt, "").expect("stt");
        std::fs::write(&tts, "").expect("tts");
        let settings = VoiceSettings {
            stt_command: stt.to_string_lossy().to_string(),
            stt_provider: "whisper".into(),
            tts_command: tts.to_string_lossy().to_string(),
            tts_provider: "espeak".into(),
            ..Default::default()
        };
        let capabilities = capabilities(&settings);
        assert!(capabilities.available);
        assert!(capabilities.stt.is_some());
        assert!(capabilities.tts.is_some());
    }
}
