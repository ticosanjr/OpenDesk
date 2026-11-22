use super::*;

use anyhow::anyhow;
use std::path::PathBuf;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    sync::mpsc,
};

pub(super) fn spawn_line_reader<R>(reader: R, source: &'static str, line_tx: mpsc::Sender<String>)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if line.starts_with("server password ") {
                        let _ = line_tx.send(line).await;
                    } else {
                        tracing::debug!(target: "opencode", source, "{line}");
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    tracing::debug!(target: "opencode", source, error = %error, "log reader stopped");
                    break;
                }
            }
        }
    });
}

/// Remove dedicated daemons left behind by a previous OpenDesk process that
/// crashed or was replaced by an update. Only processes explicitly tagged by
/// OpenDesk are touched; the user's own `opencode serve` is never killed.
pub fn cleanup_orphaned_daemons() {
    #[cfg(target_os = "linux")]
    {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return;
        };
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            let Ok(pid) = name.parse::<u32>() else {
                continue;
            };
            if pid == std::process::id() {
                continue;
            }
            let Ok(bytes) = std::fs::read(entry.path().join("environ")) else {
                continue;
            };
            let env = String::from_utf8_lossy(&bytes);
            if !env.split('\0').any(|value| value == "OPENDESK_DAEMON=1") {
                continue;
            }
            let parent_alive = env
                .split('\0')
                .find_map(|value| value.strip_prefix("OPENDESK_PARENT_PID="))
                .and_then(|value| value.parse::<u32>().ok())
                .map(|parent| std::path::Path::new(&format!("/proc/{parent}")).exists())
                .unwrap_or(false);
            if parent_alive {
                continue;
            }
            tracing::warn!(pid, "terminating orphaned OpenDesk daemon");
            let _ = std::process::Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .status();
        }
    }
}

pub(super) fn enriched_path(binary: &str) -> String {
    let mut paths = Vec::new();
    if let Some(parent) = std::path::Path::new(binary).parent() {
        paths.push(parent.to_path_buf());
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        paths.push(home.join(".opencode/bin"));
        paths.push(home.join(".local/bin"));
        paths.push(home.join(".bun/bin"));
    }
    #[cfg(windows)]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        paths.push(PathBuf::from(appdata).join("npm"));
    }
    if let Some(current) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&current));
    }
    paths.push(PathBuf::from("/usr/local/bin"));
    paths.push(PathBuf::from("/usr/bin"));
    paths.push(PathBuf::from("/bin"));
    std::env::join_paths(paths)
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|_| binary.to_string())
}

pub(super) fn service_state_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(state) = std::env::var_os("XDG_STATE_HOME") {
        paths.push(PathBuf::from(state).join("opencode/service.json"));
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        paths.push(home.join(".local/state/opencode/service.json"));
        paths.push(home.join(".config/opencode/service.json"));
    }
    paths
}

pub(super) fn find_free_port() -> Result<u16> {
    for port in DEFAULT_PORT..=LAST_PORT {
        if std::net::TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return Ok(port);
        }
    }
    Err(anyhow!(
        "no free port in {DEFAULT_PORT}-{LAST_PORT} for the OpenCode daemon"
    ))
}

pub(super) fn read_lock_file(path: &std::path::Path) -> Option<DaemonLock> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(super) fn write_lock_file(path: &std::path::Path, lock: &DaemonLock) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(lock)?;
    std::fs::write(path, bytes)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }

    Ok(())
}
