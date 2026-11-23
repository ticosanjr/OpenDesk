//! Voice input/output through host engines.
//!
//! OpenDesk does not bundle a model: transcription delegates to `whisper-cli`
//! (or a configured command) and speech to `piper`, `espeak-ng`, `espeak` or
//! macOS `say`. Commands are templates with `{audio}`, `{out_base}`, `{out_audio}`,
//! `{text}`, `{voice}` and `{language}` placeholders.

use crate::{
    db::settings,
    error::{AppError, AppResult},
    paths::AppPaths,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{path::Path, process::Stdio, time::Duration};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

pub const VOICE_SETTING: &str = "voice_settings";
const MAX_AUDIO_BYTES: usize = 25 * 1024 * 1024;
const MAX_TEXT: usize = 4_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VoiceSettings {
    pub stt_command: String,
    pub stt_args: Vec<String>,
    pub tts_command: String,
    pub tts_args: Vec<String>,
    pub language: String,
    pub voice: String,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        let stt = detect(&["whisper-cli", "whisper"]).unwrap_or_default();
        let tts = detect(&["piper", "espeak-ng", "espeak", "say"]).unwrap_or_default();
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
            tts_command: tts,
            tts_args: vec!["{text}".into()],
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
}

pub fn capabilities(settings: &VoiceSettings) -> VoiceCapabilities {
    let stt = if settings.stt_command.trim().is_empty() {
        detect(&["whisper-cli", "whisper"])
    } else {
        Some(settings.stt_command.clone()).filter(|command| exists(command))
    };
    let tts = if settings.tts_command.trim().is_empty() {
        detect(&["piper", "espeak-ng", "espeak", "say"])
    } else {
        Some(settings.tts_command.clone()).filter(|command| exists(command))
    };
    let reason = match (&stt, &tts) {
        (None, None) => Some("instale whisper e piper/espeak para voz".into()),
        (None, _) => Some("nenhum motor de transcrição encontrado".into()),
        (_, None) => Some("nenhum motor de fala encontrado".into()),
        _ => None,
    };
    VoiceCapabilities {
        available: stt.is_some() && tts.is_some(),
        stt,
        tts,
        reason,
    }
}

fn detect(commands: &[&str]) -> Option<String> {
    commands
        .iter()
        .find(|command| exists(command))
        .map(|command| (*command).to_string())
}

fn exists(command: &str) -> bool {
    if command.contains('/') {
        return Path::new(command).is_file();
    }
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(command).is_file()))
        .unwrap_or(false)
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

fn render(template: &[String], values: &[(&str, &str)]) -> Vec<String> {
    template
        .iter()
        .map(|arg| {
            let mut value = arg.clone();
            for (key, replacement) in values {
                value = value.replace(&format!("{{{key}}}"), replacement);
            }
            value
        })
        .collect()
}

#[derive(Debug, Serialize)]
pub struct Transcription {
    pub text: String,
    pub engine: String,
}

pub async fn transcribe(
    pool: &SqlitePool,
    paths: &AppPaths,
    audio_base64: &str,
    extension: &str,
) -> AppResult<Transcription> {
    let settings = get(pool).await?;
    let capabilities = capabilities(&settings);
    let Some(engine) = capabilities.stt else {
        return Err(AppError::Conflict(
            capabilities
                .reason
                .unwrap_or_else(|| "transcrição indisponível".into()),
        ));
    };
    let bytes = STANDARD
        .decode(audio_base64)
        .map_err(|_| AppError::BadRequest("áudio base64 inválido".into()))?;
    if bytes.is_empty() || bytes.len() > MAX_AUDIO_BYTES {
        return Err(AppError::BadRequest("áudio vazio ou acima de 25 MB".into()));
    }
    let extension = match extension.trim().trim_start_matches('.') {
        "mp3" | "ogg" | "webm" | "m4a" | "wav" | "mp4" => extension.trim().trim_start_matches('.'),
        _ => "wav",
    };
    let dir = paths.data_dir.join("voice");
    tokio::fs::create_dir_all(&dir).await?;
    let id = Uuid::now_v7().to_string();
    let audio = dir.join(format!("{id}.{extension}"));
    let out_base = dir.join(&id);
    tokio::fs::write(&audio, &bytes).await?;
    let out_base_text = out_base.to_string_lossy().to_string();
    let args = render(
        &settings.stt_args,
        &[
            ("audio", &audio.to_string_lossy()),
            ("out_base", &out_base_text),
            ("language", &settings.language),
            ("voice", &settings.voice),
        ],
    );
    let mut command = tokio::process::Command::new(&engine);
    command.args(&args).kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let output = tokio::time::timeout(Duration::from_secs(10 * 60), command.output())
        .await
        .map_err(|_| AppError::Internal("transcrição expirou".into()))??;
    let _ = tokio::fs::remove_file(&audio).await;
    if !output.status.success() {
        return Err(AppError::Internal(format!(
            "{engine} falhou: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .next()
                .unwrap_or("")
        )));
    }

    let txt_path = std::path::PathBuf::from(format!("{out_base_text}.txt"));
    let text = if txt_path.is_file() {
        let text = tokio::fs::read_to_string(&txt_path).await?;
        let _ = tokio::fs::remove_file(&txt_path).await;
        text
    } else {
        String::from_utf8_lossy(&output.stdout).to_string()
    };
    Ok(Transcription {
        text: text.trim().chars().take(MAX_TEXT).collect(),
        engine,
    })
}

#[derive(Debug, Serialize)]
pub struct Speech {
    pub path: String,
    pub mime: String,
    pub data: String,
}

pub async fn speak(
    pool: &SqlitePool,
    paths: &AppPaths,
    workspace_root: &Path,
    text: &str,
) -> AppResult<Speech> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > MAX_TEXT {
        return Err(AppError::BadRequest(
            "texto vazio ou acima de 4000 caracteres".into(),
        ));
    }
    let settings = get(pool).await?;
    let capabilities = capabilities(&settings);
    let Some(engine) = capabilities.tts else {
        return Err(AppError::Conflict(
            capabilities
                .reason
                .unwrap_or_else(|| "fala indisponível".into()),
        ));
    };
    let dir = workspace_root
        .join("output")
        .join(chrono::Utc::now().format("%Y-%m").to_string())
        .join("voice");
    tokio::fs::create_dir_all(&dir).await?;
    let id = Uuid::now_v7().to_string();
    let out = dir.join(format!("{id}.wav"));
    let out_audio = out.to_string_lossy().to_string();

    // Known engines get pragmatic defaults; a custom command still uses the
    // configured template and receives text on stdin when it has no {text}.
    let args = if engine == "piper" {
        vec!["--output_file".to_string(), out_audio.clone()]
    } else if engine == "espeak" || engine == "espeak-ng" {
        vec!["-w".into(), out_audio.clone(), text.to_string()]
    } else if engine == "say" {
        vec!["-o".into(), out_audio.clone(), text.to_string()]
    } else {
        render(
            &settings.tts_args,
            &[
                ("text", text),
                ("out_audio", &out_audio),
                ("voice", &settings.voice),
                ("language", &settings.language),
            ],
        )
    };

    let mut command = tokio::process::Command::new(&engine);
    command
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    if !settings.voice.trim().is_empty() && engine != "piper" {
        command.arg("-v").arg(settings.voice.trim());
    }
    let mut child = command
        .spawn()
        .map_err(|error| AppError::Internal(format!("não foi possível falar: {error}")))?;
    let needs_stdin =
        engine == "piper" || !settings.tts_args.iter().any(|arg| arg.contains("{text}"));
    if needs_stdin && let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes()).await;
    }
    let output = tokio::time::timeout(Duration::from_secs(120), child.wait_with_output())
        .await
        .map_err(|_| AppError::Internal("fala expirou".into()))??;
    let _ = paths;
    if !output.status.success() {
        return Err(AppError::Internal(format!(
            "{engine} falhou: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .next()
                .unwrap_or("")
        )));
    }
    let bytes = tokio::fs::read(&out).await.map_err(|error| {
        AppError::Internal(format!("o motor de fala não gerou o arquivo: {error}"))
    })?;
    Ok(Speech {
        path: out_audio,
        mime: "audio/wav".into(),
        data: STANDARD.encode(bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_placeholders() {
        let args = render(
            &["-f".into(), "{audio}".into(), "{language}".into()],
            &[("audio", "/tmp/a.wav"), ("language", "pt")],
        );
        assert_eq!(args, vec!["-f", "/tmp/a.wav", "pt"]);
    }

    #[tokio::test]
    async fn persists_settings_and_reports_missing_engines() {
        let ctx = crate::test_support::context().await;
        let saved = set(
            &ctx.state.db,
            VoiceSettings {
                stt_command: "definitely-missing-stt".into(),
                tts_command: "definitely-missing-tts".into(),
                language: "en".into(),
                ..Default::default()
            },
        )
        .await
        .expect("set");
        assert_eq!(saved.language, "en");
        let loaded = get(&ctx.state.db).await.expect("get");
        assert_eq!(loaded.stt_command, "definitely-missing-stt");
        assert!(!capabilities(&loaded).available);
    }

    #[test]
    fn capabilities_accepts_existing_commands() {
        let temp = tempfile::TempDir::new().expect("temp");
        let stt = temp.path().join("stt");
        let tts = temp.path().join("tts");
        std::fs::write(&stt, "").expect("stt");
        std::fs::write(&tts, "").expect("tts");
        let settings = VoiceSettings {
            stt_command: stt.to_string_lossy().to_string(),
            tts_command: tts.to_string_lossy().to_string(),
            ..Default::default()
        };
        let capabilities = capabilities(&settings);
        assert!(capabilities.available);
        assert!(capabilities.stt.is_some());
        assert!(capabilities.tts.is_some());
    }
}
