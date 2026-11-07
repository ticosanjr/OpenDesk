use crate::{
    app_state::AppState,
    chat,
    db::{chat as chat_db, settings},
    error::{AppError, AppResult},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{path::PathBuf, process::Stdio, sync::Arc};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::Mutex,
};

const SETTING_KEY: &str = "signal_config";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub account: String,
    #[serde(default = "default_cli")]
    pub cli_path: String,
    #[serde(default)]
    pub allowlist: Vec<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default = "default_agent")]
    pub agent: String,
}

fn default_cli() -> String {
    default_cli_path()
}

fn default_agent() -> String {
    "build".into()
}

impl Default for SignalConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            account: String::new(),
            cli_path: default_cli(),
            allowlist: Vec::new(),
            workspace_id: None,
            agent: default_agent(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SignalStatus {
    pub config: SignalConfig,
    pub cli_available: bool,
    pub cli_version: Option<String>,
    pub bridge_running: bool,
    pub link_running: bool,
}

#[derive(Default)]
pub struct SignalBridge {
    handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    link_child: Mutex<Option<Child>>,
}

pub async fn load(pool: &sqlx::SqlitePool) -> AppResult<SignalConfig> {
    let value = settings::get(pool, SETTING_KEY).await?;
    Ok(value
        .and_then(|value| serde_json::from_value::<SignalConfig>(value).ok())
        .unwrap_or_default())
}

pub async fn save(pool: &sqlx::SqlitePool, config: &SignalConfig) -> AppResult<()> {
    settings::set(pool, SETTING_KEY, &serde_json::to_value(config)?).await
}

pub async fn detect_cli(path: &str) -> Option<String> {
    let output = Command::new(path)
        .arg("--version")
        .env("PATH", signal_path())
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub async fn status(state: &AppState) -> SignalStatus {
    let config = load(&state.db).await.unwrap_or_default();
    let cli_version = detect_cli(&config.cli_path).await;
    SignalStatus {
        cli_available: cli_version.is_some(),
        cli_version,
        bridge_running: state.signal.is_running().await,
        link_running: state.signal.is_linking().await,
        config,
    }
}

impl SignalBridge {
    pub async fn is_running(&self) -> bool {
        self.handle
            .lock()
            .await
            .as_ref()
            .map(|handle| !handle.is_finished())
            .unwrap_or(false)
    }

    pub async fn is_linking(&self) -> bool {
        let mut child = self.link_child.lock().await;
        match child.as_mut() {
            Some(value) => match value.try_wait() {
                Ok(Some(_)) => {
                    *child = None;
                    false
                }
                Ok(None) => true,
                Err(_) => {
                    *child = None;
                    false
                }
            },
            None => false,
        }
    }

    pub async fn stop(&self) {
        if let Some(handle) = self.handle.lock().await.take() {
            handle.abort();
        }
        let mut link = self.link_child.lock().await;
        if let Some(mut child) = link.take() {
            let _ = child.kill().await;
        }
    }

    pub async fn start_if_enabled(self: &Arc<Self>, state: Arc<AppState>) -> AppResult<bool> {
        self.stop().await;
        let config = load(&state.db).await?;
        if !config.enabled {
            return Ok(false);
        }
        if config.account.trim().is_empty() {
            return Err(AppError::BadRequest(
                "Signal está habilitado, mas a conta (número) não foi configurada.".into(),
            ));
        }
        if config.allowlist.is_empty() {
            return Err(AppError::BadRequest(
                "Signal está habilitado, mas a allowlist está vazia. Adicione ao menos um número."
                    .into(),
            ));
        }
        if detect_cli(&config.cli_path).await.is_none() {
            return Err(AppError::BadRequest(format!(
                "signal-cli não encontrado em '{}'. Instale ou informe o caminho.",
                config.cli_path
            )));
        }

        let bridge = Arc::clone(self);
        let handle = tokio::spawn(async move {
            if let Err(error) = bridge.run(state).await {
                tracing::error!(error = %error, "Signal bridge stopped");
            }
        });
        *self.handle.lock().await = Some(handle);
        Ok(true)
    }

    async fn run(self: Arc<Self>, state: Arc<AppState>) -> anyhow::Result<()> {
        let config = load(&state.db).await?;
        let mut child = Command::new(&config.cli_path)
            .args(["-a", &config.account, "--output", "json", "daemon"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| anyhow::anyhow!("could not start signal-cli: {error}"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("signal-cli stdout unavailable"))?;
        let mut lines = BufReader::new(stdout).lines();
        while let Some(line) = lines.next_line().await? {
            let Some(incoming) = parse_incoming(&line) else {
                continue;
            };
            let config = load(&state.db).await?;
            if incoming.source == config.account {
                continue;
            }
            if !allowed(&config.allowlist, &incoming.source) {
                tracing::warn!(source = %incoming.source, "Signal message rejected by allowlist");
                continue;
            }
            if let Err(error) = handle_incoming(&state, &config, incoming).await {
                tracing::warn!(error = %error, "Signal message failed");
            }
        }
        let _ = child.wait().await;
        Ok(())
    }

    /// Start `signal-cli link` and return the `sgnl://` URI to render as a QR
    /// code. The process stays alive so the phone can complete the link.
    pub async fn link(&self, cli_path: &str) -> AppResult<String> {
        let mut link = self.link_child.lock().await;
        if let Some(mut child) = link.take() {
            let _ = child.kill().await;
        }
        let mut child = Command::new(cli_path)
            .args(["link", "-n", "OpenDesk"])
            .env("PATH", signal_path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                AppError::BadRequest(format!("could not run signal-cli link: {error}"))
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::Internal("signal-cli link stdout unavailable".into()))?;
        let mut lines = BufReader::new(stdout).lines();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(15);
        let mut uri = None;
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(std::time::Duration::from_secs(2), lines.next_line()).await {
                Ok(Ok(Some(line))) => {
                    if let Some(index) = line.find("sgnl://") {
                        uri = Some(line[index..].trim().to_string());
                        break;
                    }
                }
                Ok(Ok(None)) => break,
                Ok(Err(error)) => {
                    let _ = child.kill().await;
                    return Err(AppError::Internal(format!(
                        "signal-cli link failed: {error}"
                    )));
                }
                Err(_) => {}
            }
        }

        let Some(uri) = uri else {
            let _ = child.kill().await;
            return Err(AppError::BadRequest(
                "signal-cli não retornou um URI de vínculo. Execute `signal-cli link -n OpenDesk` no terminal e escaneie o QR."
                    .into(),
            ));
        };

        // Keep draining the process output so it does not block while waiting
        // for the phone to scan the QR code.
        tokio::spawn(async move {
            let mut lines = lines;
            while let Ok(Ok(Some(_))) =
                tokio::time::timeout(std::time::Duration::from_secs(60), lines.next_line()).await
            {
            }
        });
        *link = Some(child);
        Ok(uri)
    }
}

async fn handle_incoming(
    state: &Arc<AppState>,
    config: &SignalConfig,
    incoming: IncomingSignal,
) -> AppResult<()> {
    if crate::recipes::hitl::signal::handle_reply(state, &incoming.source, &incoming.text, &[])
        .await?
    {
        return Ok(());
    }
    let trimmed = incoming.text.trim();
    let lower = trimmed.to_lowercase();
    if let Some(rest) = lower
        .strip_prefix("aprovar ")
        .or_else(|| lower.strip_prefix("negar "))
    {
        let approve = lower.starts_with("aprovar ");
        let prefix = rest.split_whitespace().next().unwrap_or_default();
        let decision = crate::db::permissions::decide_prefix(
            &state.db,
            prefix,
            if approve { "once" } else { "reject" },
        )
        .await?;
        let message = match decision {
            Some(_) if approve => format!("Permissão {prefix} aprovada."),
            Some(_) => format!("Permissão {prefix} negada."),
            None => format!("Nenhum pedido pendente com o prefixo {prefix}."),
        };
        send_text(
            &config.cli_path,
            &config.account,
            &incoming.source,
            &message,
        )
        .await?;
        return Ok(());
    }

    let workspace_id = match &config.workspace_id {
        Some(id) => id.clone(),
        None => settings::active_workspace_id(&state.db)
            .await?
            .ok_or_else(|| AppError::BadRequest("no active workspace for Signal".into()))?,
    };
    let thread =
        match chat_db::find_external(&state.db, &workspace_id, "signal", &incoming.source).await? {
            Some(thread) => thread,
            None => {
                chat_db::create_thread(
                    &state.db,
                    chat_db::CreateThreadInput {
                        workspace_id: workspace_id.clone(),
                        title: Some(format!("Signal · {}", incoming.source)),
                        agent: config.agent.clone(),
                        model: None,
                        source: Some("signal".into()),
                        external_id: Some(incoming.source.clone()),
                    },
                )
                .await?
            }
        };

    let reply = chat::send_message(state, thread, &incoming.text).await?;
    send_text(
        &config.cli_path,
        &config.account,
        &incoming.source,
        &reply.assistant.content,
    )
    .await?;
    Ok(())
}

pub async fn send_text(
    cli_path: &str,
    account: &str,
    recipient: &str,
    message: &str,
) -> AppResult<()> {
    send_with_attachments(cli_path, account, recipient, message, &[]).await
}

pub async fn send_with_attachments(
    cli_path: &str,
    account: &str,
    recipient: &str,
    message: &str,
    attachments: &[String],
) -> AppResult<()> {
    let mut command = Command::new(cli_path);
    command.args(["-a", account, "send", recipient, "-m", message]);
    for attachment in attachments {
        command.arg("--attachment").arg(attachment);
    }
    let output = command
        .env("PATH", signal_path())
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|error| AppError::Daemon(format!("could not run signal-cli send: {error}")))?;
    if !output.status.success() {
        return Err(AppError::Daemon(format!(
            "signal-cli send failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingSignal {
    pub source: String,
    pub text: String,
}

pub fn parse_incoming(line: &str) -> Option<IncomingSignal> {
    let value: Value = serde_json::from_str(line).ok()?;
    let envelope = value
        .pointer("/params/envelope")
        .or_else(|| value.get("envelope"))
        .unwrap_or(&value);
    let source = envelope
        .get("sourceNumber")
        .or_else(|| envelope.get("source"))
        .or_else(|| value.pointer("/params/source"))
        .and_then(Value::as_str)?
        .to_string();
    let message = envelope
        .pointer("/dataMessage/message")
        .or_else(|| value.pointer("/params/dataMessage/message"))
        .or_else(|| value.get("message"));
    let text = message
        .and_then(|message| {
            message
                .get("message")
                .and_then(Value::as_str)
                .or_else(|| message.as_str())
        })
        .map(str::trim)
        .filter(|text| !text.is_empty())?
        .to_string();
    Some(IncomingSignal { source, text })
}

pub fn allowed(allowlist: &[String], source: &str) -> bool {
    allowlist
        .iter()
        .map(|value| normalize_number(value))
        .any(|value| value == normalize_number(source))
}

fn normalize_number(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_digit())
        .collect()
}

/// signal-cli 0.14 requires a recent JVM. AppImages often inherit a minimal
/// PATH, so prepend the common Java installations before spawning it.
pub fn signal_path() -> String {
    let mut paths = Vec::new();
    for candidate in ["/opt/java/current/bin", "/usr/lib/jvm/default-java/bin"] {
        let path = PathBuf::from(candidate);
        if path.is_dir() {
            paths.push(path);
        }
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let sdkman = home.join(".sdkman/candidates/java/current/bin");
        if sdkman.is_dir() {
            paths.push(sdkman);
        }
    }
    if let Some(current) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&current));
    }
    std::env::join_paths(paths)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| "java".into())
}

pub fn default_cli_path() -> String {
    std::env::var("SIGNAL_CLI_PATH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            let candidate = PathBuf::from(std::env::var("HOME").unwrap_or_default())
                .join(".local/bin/signal-cli");
            if candidate.is_file() {
                candidate.to_string_lossy().to_string()
            } else {
                "signal-cli".into()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_rpc_and_daemon_envelopes() {
        let json_rpc = r#"{"jsonrpc":"2.0","method":"receive","params":{"envelope":{"sourceNumber":"+15551234567","dataMessage":{"message":"Olá"}}}}"#;
        assert_eq!(
            parse_incoming(json_rpc),
            Some(IncomingSignal {
                source: "+15551234567".into(),
                text: "Olá".into()
            })
        );

        let daemon =
            r#"{"envelope":{"source":"+15557654321","dataMessage":{"message":{"message":"Oi"}}}}"#;
        assert_eq!(
            parse_incoming(daemon),
            Some(IncomingSignal {
                source: "+15557654321".into(),
                text: "Oi".into()
            })
        );
        assert!(parse_incoming("not json").is_none());
        assert!(parse_incoming(r#"{"envelope":{"sourceNumber":"+1"}}"#).is_none());
    }

    #[tokio::test]
    async fn link_extracts_the_signal_uri() {
        let temp = tempfile::TempDir::new().expect("temp");
        let script = temp.path().join("signal");
        std::fs::write(
            &script,
            "#!/bin/sh\necho 'sgnl://linkdevice?uuid=abc&id=xyz'\nsleep 5\n",
        )
        .expect("script");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
                .expect("chmod");
        }
        let bridge = SignalBridge::default();
        let uri = bridge
            .link(&script.to_string_lossy())
            .await
            .expect("link uri");
        assert!(uri.starts_with("sgnl://linkdevice"));
        bridge.stop().await;
    }

    #[test]
    fn signal_path_is_never_empty() {
        assert!(!signal_path().is_empty());
    }

    #[test]
    fn allowlist_compares_digits_only() {
        assert!(allowed(&["+55 (11) 99999-0000".into()], "+5511999990000"));
        assert!(!allowed(&["+5511999990000".into()], "+5511888880000"));
        assert!(!allowed(&[], "+5511999990000"));
    }
}
