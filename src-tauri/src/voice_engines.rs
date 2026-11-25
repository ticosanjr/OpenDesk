//! Host engines and OpenAI-compatible endpoints for speech synthesis and
//! transcription. Every child process is killed on drop and bounded by a 60s
//! timeout; nothing is executed through a shell.

use crate::{
    error::{AppError, AppResult},
    paths::AppPaths,
};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

use super::{VoiceSettings, voice_http};

pub(super) struct AudioOut {
    pub bytes: Vec<u8>,
    pub mime: String,
    pub provider: String,
}

const TIMEOUT: Duration = Duration::from_secs(60);
const MAX_AUDIO_BYTES: usize = 25 * 1024 * 1024;

fn exists(command: &str) -> bool {
    if command.contains('/') {
        return Path::new(command).is_file();
    }
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(command).is_file()))
        .unwrap_or(false)
}

pub(super) fn detect(commands: &[&str]) -> Option<String> {
    commands
        .iter()
        .find(|command| exists(command))
        .map(|command| (*command).to_string())
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

pub(super) fn provider_order(raw: &str, automatic: &[&str]) -> Vec<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("auto") {
        return automatic.iter().map(|item| (*item).to_string()).collect();
    }
    raw.split(',')
        .map(|item| item.trim().to_lowercase())
        .map(|item| match item.as_str() {
            "endpoint" | "openai-compatible" | "openai_compatible" => "openai".into(),
            "whisper-cli" | "whisper.cpp" | "whisper_cpp" => "whisper".into(),
            "espeak-ng" | "espeak_ng" => "espeak".into(),
            other => other.to_string(),
        })
        .filter(|item| !item.is_empty())
        .collect()
}

fn engine_binary(settings: &VoiceSettings, kind: &str, names: &[&str]) -> Option<String> {
    let configured = match kind {
        "tts" => settings.tts_command.trim(),
        _ => settings.stt_command.trim(),
    };
    if !configured.is_empty() {
        let base = Path::new(configured)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or(configured);
        if names.contains(&base) && exists(configured) {
            return Some(configured.to_string());
        }
    }
    detect(names)
}

fn endpoint_ready(endpoint: &str, secret_ref: &str) -> bool {
    !endpoint.trim().is_empty() && !secret_ref.trim().is_empty()
}

pub(super) fn tts_available(settings: &VoiceSettings) -> bool {
    provider_order(
        &settings.tts_provider,
        &["piper", "espeak", "openai", "custom"],
    )
    .iter()
    .any(|provider| match provider.as_str() {
        "piper" => engine_binary(settings, "tts", &["piper"]).is_some(),
        "espeak" => engine_binary(settings, "tts", &["espeak-ng", "espeak"]).is_some(),
        "openai" => endpoint_ready(&settings.tts_endpoint, &settings.tts_secret_ref),
        "custom" | "say" => exists(settings.tts_command.trim()),
        _ => false,
    })
}

pub(super) fn stt_available(settings: &VoiceSettings) -> bool {
    provider_order(&settings.stt_provider, &["whisper", "openai"])
        .iter()
        .any(|provider| match provider.as_str() {
            "whisper" => engine_binary(settings, "stt", &["whisper-cli", "whisper"]).is_some(),
            "openai" => endpoint_ready(&settings.stt_endpoint, &settings.stt_secret_ref),
            _ => false,
        })
}

async fn run(
    command: &str,
    args: &[String],
    text: Option<&str>,
) -> AppResult<std::process::Output> {
    let mut child = tokio::process::Command::new(command);
    child
        .args(args)
        .stdin(if text.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    child.process_group(0);
    let mut child = child.spawn().map_err(|error| {
        AppError::Internal(format!("não foi possível executar {command}: {error}"))
    })?;
    if let Some(text) = text
        && let Some(mut stdin) = child.stdin.take()
    {
        let _ = stdin.write_all(text.as_bytes()).await;
    }
    let result = tokio::time::timeout(TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| AppError::Internal(format!("{command} excedeu 60s")))??;
    if !result.status.success() {
        return Err(AppError::Internal(format!(
            "{command} falhou: {}",
            String::from_utf8_lossy(&result.stderr)
                .lines()
                .next()
                .unwrap_or("")
        )));
    }
    Ok(result)
}

async fn piper_speech(
    settings: &VoiceSettings,
    paths: &AppPaths,
    binary: &str,
    text: &str,
) -> AppResult<AudioOut> {
    let dir = paths.data_dir.join("voice");
    tokio::fs::create_dir_all(&dir).await?;
    let out = dir.join(format!("{}.wav", Uuid::now_v7()));
    let out_text = out.to_string_lossy().to_string();
    let mut args = render(
        &settings.tts_args,
        &[
            ("text", text),
            ("out_audio", &out_text),
            ("voice", &settings.voice),
            ("language", &settings.language),
        ],
    );
    if args.len() == 1 && args[0] == text {
        args.clear();
    }
    let has_output = args.iter().any(|arg| arg.contains("--output_file"));
    if !has_output {
        args.push("--output_file".into());
        args.push(out_text.clone());
    }
    if !settings.tts_model.trim().is_empty()
        && !args.iter().any(|arg| arg == "--model" || arg == "-m")
    {
        args.push("--model".into());
        args.push(settings.tts_model.trim().to_string());
    }
    let output = run(binary, &args, Some(text)).await?;
    let bytes = tokio::fs::read(&out).await.map_err(|_| {
        AppError::Internal(format!(
            "piper não gerou áudio: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .next()
                .unwrap_or("")
        ))
    })?;
    let _ = tokio::fs::remove_file(&out).await;
    Ok(AudioOut {
        bytes,
        mime: "audio/wav".into(),
        provider: "piper".into(),
    })
}

async fn espeak_speech(settings: &VoiceSettings, binary: &str, text: &str) -> AppResult<AudioOut> {
    let mut args = vec!["--stdout".to_string()];
    if !settings.voice.trim().is_empty() {
        args.push("-v".into());
        args.push(settings.voice.trim().to_string());
    }
    args.push(text.to_string());
    let output = run(binary, &args, None).await?;
    if !output.stdout.is_empty() {
        return Ok(AudioOut {
            bytes: output.stdout,
            mime: "audio/wav".into(),
            provider: binary.split('/').next_back().unwrap_or("espeak").into(),
        });
    }
    Err(AppError::Internal("espeak não retornou áudio".into()))
}

async fn custom_speech(
    settings: &VoiceSettings,
    paths: &AppPaths,
    text: &str,
) -> AppResult<AudioOut> {
    let binary = settings.tts_command.trim();
    if binary.is_empty() {
        return Err(AppError::Internal("comando TTS vazio".into()));
    }
    let dir = paths.data_dir.join("voice");
    tokio::fs::create_dir_all(&dir).await?;
    let out = dir.join(format!("{}.wav", Uuid::now_v7()));
    let out_text = out.to_string_lossy().to_string();
    let args = render(
        &settings.tts_args,
        &[
            ("text", text),
            ("out_audio", &out_text),
            ("voice", &settings.voice),
            ("language", &settings.language),
        ],
    );
    let mut command = tokio::process::Command::new(binary);
    command
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command
        .spawn()
        .map_err(|error| AppError::Internal(format!("TTS: {error}")))?;
    if !args.iter().any(|arg| arg.contains("{text}"))
        && args.iter().all(|arg| arg != text)
        && let Some(mut stdin) = child.stdin.take()
    {
        let _ = stdin.write_all(text.as_bytes()).await;
    }
    let output = tokio::time::timeout(TIMEOUT, child.wait_with_output())
        .await
        .map_err(|_| AppError::Internal("TTS excedeu 60s".into()))??;
    if !output.status.success() {
        return Err(AppError::Internal(format!(
            "{binary} falhou: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .next()
                .unwrap_or("")
        )));
    }
    if let Ok(bytes) = tokio::fs::read(&out).await {
        let _ = tokio::fs::remove_file(&out).await;
        return Ok(AudioOut {
            bytes,
            mime: "audio/wav".into(),
            provider: "custom".into(),
        });
    }
    if !output.stdout.is_empty() {
        return Ok(AudioOut {
            bytes: output.stdout,
            mime: "audio/wav".into(),
            provider: "custom".into(),
        });
    }
    Err(AppError::Internal("comando TTS não gerou áudio".into()))
}

pub(super) async fn speak(
    settings: &VoiceSettings,
    paths: &AppPaths,
    text: &str,
) -> AppResult<AudioOut> {
    let order = provider_order(
        &settings.tts_provider,
        &["piper", "espeak", "openai", "custom"],
    );
    for provider in &order {
        match provider.as_str() {
            "piper" => {
                if let Some(binary) = engine_binary(settings, "tts", &["piper"]) {
                    return piper_speech(settings, paths, &binary, text).await;
                }
            }
            "espeak" => {
                if let Some(binary) = engine_binary(settings, "tts", &["espeak-ng", "espeak"]) {
                    return espeak_speech(settings, &binary, text).await;
                }
            }
            "openai" => {
                if endpoint_ready(&settings.tts_endpoint, &settings.tts_secret_ref) {
                    return voice_http::endpoint_speech(settings, paths, text).await;
                }
            }
            "custom" | "say" if exists(settings.tts_command.trim()) => {
                return custom_speech(settings, paths, text).await;
            }
            _ => {}
        }
    }
    Err(AppError::Unavailable(
        "Nenhum motor TTS disponível.".into(),
        "Instale piper ou espeak-ng (`sudo apt install espeak-ng`), configure um comando TTS ou um endpoint OpenAI-compatible com secret_ref.".into(),
    ))
}

async fn convert_to_wav(
    paths: &AppPaths,
    source: &Path,
    id: &str,
) -> AppResult<std::path::PathBuf> {
    let ffmpeg = detect(&["ffmpeg"]).ok_or_else(|| {
        AppError::Unavailable(
            "ffmpeg não encontrado para converter a gravação.".into(),
            "Instale ffmpeg (`sudo apt install ffmpeg`) ou configure um endpoint STT que aceite webm/ogg.".into(),
        )
    })?;
    let target = paths.data_dir.join("voice").join(format!("{id}.wav"));
    let args = vec![
        "-y".into(),
        "-i".into(),
        source.to_string_lossy().to_string(),
        "-ar".into(),
        "16000".into(),
        "-ac".into(),
        "1".into(),
        target.to_string_lossy().to_string(),
    ];
    run(&ffmpeg, &args, None).await?;
    Ok(target)
}

async fn whisper_transcribe(
    settings: &VoiceSettings,
    paths: &AppPaths,
    bytes: &[u8],
    mime: &str,
) -> AppResult<(String, String)> {
    let Some(binary) = engine_binary(settings, "stt", &["whisper-cli", "whisper"]) else {
        return Err(AppError::Internal("whisper não encontrado".into()));
    };
    let dir = paths.data_dir.join("voice");
    tokio::fs::create_dir_all(&dir).await?;
    let id = Uuid::now_v7().to_string();
    let extension = match mime.rsplit('/').next().unwrap_or("wav") {
        "mpeg" | "mp3" => "mp3",
        "ogg" => "ogg",
        "webm" => "webm",
        "mp4" | "m4a" => "m4a",
        _ => "wav",
    };
    let source = dir.join(format!("{id}.{extension}"));
    tokio::fs::write(&source, bytes).await?;
    let audio = if extension == "wav" {
        source.clone()
    } else {
        convert_to_wav(paths, &source, &id).await?
    };
    let out_base = dir.join(&id);
    let out_text = out_base.to_string_lossy().to_string();
    let args = render(
        &settings.stt_args,
        &[
            ("audio", &audio.to_string_lossy()),
            ("out_base", &out_text),
            ("language", &settings.language),
            ("voice", &settings.voice),
        ],
    );
    let output = run(&binary, &args, None).await;
    let _ = tokio::fs::remove_file(&source).await;
    let _ = tokio::fs::remove_file(&audio).await;
    let output = output?;
    let txt = std::path::PathBuf::from(format!("{out_text}.txt"));
    let text = if txt.is_file() {
        let text = tokio::fs::read_to_string(&txt).await?;
        let _ = tokio::fs::remove_file(&txt).await;
        text
    } else {
        String::from_utf8_lossy(&output.stdout).to_string()
    };
    Ok((
        text,
        binary.split('/').next_back().unwrap_or("whisper").into(),
    ))
}

pub(super) async fn transcribe(
    settings: &VoiceSettings,
    paths: &AppPaths,
    bytes: &[u8],
    mime: &str,
) -> AppResult<(String, String)> {
    if bytes.len() < 1024 {
        return Err(AppError::BadRequest(
            "áudio vazio ou curto demais (mínimo 1 KB)".into(),
        ));
    }
    if bytes.len() > MAX_AUDIO_BYTES {
        return Err(AppError::BadRequest("áudio acima de 25 MB".into()));
    }
    let order = provider_order(&settings.stt_provider, &["whisper", "openai"]);
    let mut last_error = None;
    for provider in &order {
        match provider.as_str() {
            "whisper" => {
                if engine_binary(settings, "stt", &["whisper-cli", "whisper"]).is_some() {
                    match whisper_transcribe(settings, paths, bytes, mime).await {
                        Ok(value) => return Ok(value),
                        Err(error) => last_error = Some(error),
                    }
                }
            }
            "openai" if endpoint_ready(&settings.stt_endpoint, &settings.stt_secret_ref) => {
                match voice_http::endpoint_transcribe(settings, paths, bytes, mime).await {
                    Ok(value) => return Ok(value),
                    Err(error) => last_error = Some(error),
                }
            }
            _ => {}
        }
    }
    if let Some(error) = last_error {
        return Err(error);
    }
    Err(AppError::Unavailable(
        "Nenhum motor STT disponível.".into(),
        "Instale whisper.cpp (whisper-cli) e ffmpeg, ou configure um endpoint OpenAI-compatible com secret_ref.".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_order_handles_aliases_and_auto() {
        assert_eq!(
            provider_order("auto", &["piper", "espeak", "openai"]),
            vec!["piper", "espeak", "openai"]
        );
        assert_eq!(
            provider_order("espeak-ng, openai-compatible, whisper-cli", &[]),
            vec!["espeak", "openai", "whisper"]
        );
    }
}
