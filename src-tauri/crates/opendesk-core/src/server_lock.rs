//! Best-effort single-instance guard for the OpenDesk backend.
//!
//! The desktop and `opendeskd` both own the SQLite database and the local API
//! port. The health probe is the primary guard; this PID file closes the small
//! window between "no daemon answered" and "embedded state is initialized".
//!
//! Liveness is only verified on Linux through `/proc`. On other platforms an
//! existing file is treated as stale because the caller already confirmed that
//! `/healthz` did not answer.

use crate::{error::AppResult, paths::AppPaths};
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};

pub const LOCK_FILE: &str = "server.pid";

pub struct ServerLock {
    path: PathBuf,
}

impl ServerLock {
    /// Returns `None` when another live OpenDesk process holds the lock.
    pub fn acquire(paths: &AppPaths) -> AppResult<Option<Self>> {
        let path = paths.data_dir.join(LOCK_FILE);
        if let Some(pid) = read_pid(&path)
            && process_alive(pid)
        {
            return Ok(None);
        }
        let _ = std::fs::remove_file(&path);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                let _ = writeln!(file, "{}", std::process::id());
                Ok(Some(Self { path }))
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn pid(paths: &AppPaths) -> Option<u32> {
        read_pid(&paths.data_dir.join(LOCK_FILE))
    }
}

impl Drop for ServerLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn read_pid(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path)
        .ok()?
        .trim()
        .parse::<u32>()
        .ok()
}

fn process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(target_os = "linux")]
    {
        Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::test_paths;

    #[test]
    fn second_acquire_is_rejected_while_the_first_holds_the_lock() {
        let dir = tempfile::tempdir().expect("temp");
        let paths = test_paths(dir.path());
        let first = ServerLock::acquire(&paths).expect("acquire").expect("lock");
        assert_eq!(ServerLock::pid(&paths), Some(std::process::id()));
        assert!(ServerLock::acquire(&paths).expect("second").is_none());
        drop(first);
        assert!(ServerLock::pid(&paths).is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn stale_pid_is_replaced() {
        let dir = tempfile::tempdir().expect("temp");
        let paths = test_paths(dir.path());
        std::fs::write(paths.data_dir.join(LOCK_FILE), "999999999\n").expect("stale");
        let lock = ServerLock::acquire(&paths).expect("acquire").expect("lock");
        assert_eq!(ServerLock::pid(&paths), Some(std::process::id()));
        drop(lock);
    }
}
