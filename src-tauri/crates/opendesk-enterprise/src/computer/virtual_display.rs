//! Isolated X11 displays for computer-use sessions.
//!
//! When available, a session gets its own Xvfb instance so the agent never
//! controls the user's desktop. Input/screenshots are still executed by
//! `xdotool`/`import`, but with `DISPLAY=:N`.

use crate::error::{AppError, AppResult};
use std::{
    collections::HashMap,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Mutex, OnceLock},
    time::Duration,
};

fn registry() -> &'static Mutex<HashMap<String, Child>> {
    static REGISTRY: OnceLock<Mutex<HashMap<String, Child>>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn available() -> bool {
    command_exists("Xvfb") && command_exists("xdotool") && command_exists("import")
}

pub fn start(session_id: &str) -> AppResult<String> {
    if !available() {
        return Err(AppError::Conflict(
            "display virtual exige Xvfb, xdotool e imagemagick (import)".into(),
        ));
    }
    for number in 90..130 {
        let lock = format!("/tmp/.X{number}-lock");
        let socket = format!("/tmp/.X11-unix/X{number}");
        if Path::new(&lock).exists() {
            let alive = std::fs::read_to_string(&lock)
                .ok()
                .and_then(|value| value.trim().parse::<u32>().ok())
                .is_some_and(|pid| Path::new(&format!("/proc/{pid}")).exists());
            if alive {
                continue;
            }
            // Stale lock/socket from a killed Xvfb: reclaim the display.
            let _ = std::fs::remove_file(&lock);
            let _ = std::fs::remove_file(&socket);
        }
        if std::env::var_os("DISPLAY").as_deref()
            == Some(std::ffi::OsStr::new(&format!(":{number}")))
        {
            continue;
        }
        let display = format!(":{number}");
        let child = Command::new("Xvfb")
            .arg(&display)
            .args(["-screen", "0", "1280x800x24", "-nolisten", "tcp"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| AppError::Internal(format!("Xvfb: {error}")))?;
        let mut ready = false;
        // Cold boots and loaded CI machines can take longer than 1.5 s.
        for _ in 0..100 {
            if Path::new(&socket).exists() {
                ready = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if !ready {
            let mut child = child;
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::Internal(format!(
                "Xvfb não abriu o display {display}"
            )));
        }
        registry()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .insert(session_id.to_string(), child);
        return Ok(display);
    }
    Err(AppError::Conflict(
        "nenhum display X11 livre entre :90 e :129".into(),
    ))
}

pub fn stop(session_id: &str) {
    let child = registry()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .remove(session_id);
    if let Some(mut child) = child {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn command_exists(command: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(command).is_file()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_and_stops_an_isolated_display() {
        if !available() {
            return;
        }
        let display = start("test-virtual-display").expect("start");
        assert!(display.starts_with(':'));
        assert!(
            registry()
                .lock()
                .expect("registry")
                .contains_key("test-virtual-display")
        );
        stop("test-virtual-display");
        assert!(
            !registry()
                .lock()
                .expect("registry")
                .contains_key("test-virtual-display")
        );
    }
}
