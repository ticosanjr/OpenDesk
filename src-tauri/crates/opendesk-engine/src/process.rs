//! Process shutdown helpers.
//!
//! Long-lived children (the OpenCode daemon, CLI harnesses) run in their own
//! process group. `kill_on_drop` is the last-resort guard; this helper asks
//! politely with SIGTERM first and escalates to SIGKILL after a bounded grace
//! period so a stuck child cannot keep ports or file locks alive.

use std::time::Duration;
use tokio::process::Child;

/// Terminates `child` (and its process group when it is a group leader).
pub async fn terminate_child(child: &mut Child, grace: Duration) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        let pid = pid as i32;
        // A negative pid targets the group created by `process_group(0)`.
        // Fall back to the child itself when the group signal is rejected.
        let group = unsafe { libc::kill(-pid, libc::SIGTERM) };
        if group != 0 {
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
        }
    }

    match tokio::time::timeout(grace, child.wait()).await {
        Ok(_) => {}
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }
    }
}

pub async fn terminate_pid(pid: u32) {
    #[cfg(unix)]
    {
        let _ = tokio::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .await;
        for _ in 0..30 {
            let alive = std::path::Path::new(&format!("/proc/{pid}")).exists();
            if !alive {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        let _ = tokio::process::Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status()
            .await;
    }

    #[cfg(windows)]
    {
        let _ = tokio::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status()
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn child_alive(pid: u32) -> bool {
        match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            Ok(stat) => !stat.contains(") Z "),
            Err(_) => false,
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn terminate_child_reaps_the_process() {
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg("sleep 30")
            .kill_on_drop(true)
            .process_group(0)
            .spawn()
            .expect("spawn");
        let pid = child.id().expect("pid");
        terminate_child(&mut child, Duration::from_millis(200)).await;
        assert!(!child_alive(pid));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn dropping_a_child_does_not_leave_it_running() {
        let child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg("sleep 30")
            .kill_on_drop(true)
            .process_group(0)
            .spawn()
            .expect("spawn");
        let pid = child.id().expect("pid");
        drop(child);
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!child_alive(pid));
    }
}
