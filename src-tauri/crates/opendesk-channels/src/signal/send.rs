//! Outbound `signal-cli send` invocation.

use super::config::signal_path;
use crate::error::{AppError, AppResult};
use std::process::Stdio;
use tokio::process::Command;

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
    command.kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
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
