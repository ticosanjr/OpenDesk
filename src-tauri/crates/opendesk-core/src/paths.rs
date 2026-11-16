use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::path::{Path, PathBuf};

/// All OpenDesk-owned directories. None of these replace native OpenCode files:
/// configuration always lives in `~/.config/opencode` and in each workspace.
/// Some paths are consumed by the watcher, backups and artifact store in later phases.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub log_dir: PathBuf,
    pub db_path: PathBuf,
    pub backups_dir: PathBuf,
    pub artifacts_dir: PathBuf,
    pub run_logs_dir: PathBuf,
    pub daemon_lock_path: PathBuf,
    pub api_lock_path: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let dirs = ProjectDirs::from("com", "opendesk", "OpenDesk")
            .context("could not resolve the platform application directories")?;

        let data_dir = std::env::var_os("OPENDESK_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| dirs.data_dir().to_path_buf());
        let config_dir = std::env::var_os("OPENDESK_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| dirs.config_dir().to_path_buf());
        let cache_dir = std::env::var_os("OPENDESK_CACHE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| dirs.cache_dir().to_path_buf());
        let log_dir = data_dir.join("logs");
        let backups_dir = data_dir.join("backups");
        let artifacts_dir = data_dir.join("artifacts");
        let run_logs_dir = data_dir.join("runs");

        for dir in [
            &data_dir,
            &config_dir,
            &cache_dir,
            &log_dir,
            &backups_dir,
            &artifacts_dir,
            &run_logs_dir,
        ] {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("could not create {}", dir.display()))?;
        }

        Ok(Self {
            db_path: data_dir.join("opendesk.db"),
            daemon_lock_path: data_dir.join("daemon.json"),
            api_lock_path: data_dir.join("api.json"),
            data_dir,
            config_dir,
            cache_dir,
            log_dir,
            backups_dir,
            artifacts_dir,
            run_logs_dir,
        })
    }

    /// Minimum deterministic layout for a workspace. Existing content is never touched.
    pub fn ensure_workspace_layout(&self, root: &Path) -> Result<()> {
        std::fs::create_dir_all(root)
            .with_context(|| format!("could not create workspace root {}", root.display()))?;
        // `.opencode/` is a native OpenCode workspace marker. Its presence makes
        // the directory a recognised OpenCode location, which the v2 API needs
        // before it returns location-scoped agents/models.
        for child in ["input", "output", ".opencode"] {
            std::fs::create_dir_all(root.join(child))
                .with_context(|| format!("could not create workspace/{child}"))?;
        }
        Ok(())
    }
}

/// Default parent directory for workspaces created from the OpenDesk UI.
/// It lives outside the app data directory so users can back it up, sync it or
/// open it in Finder/Explorer normally.
pub fn default_workspaces_root() -> PathBuf {
    if let Ok(configured) = std::env::var("OPENDESK_WORKSPACES_ROOT")
        && !configured.trim().is_empty()
    {
        return PathBuf::from(configured);
    }
    if let Some(user_dirs) = directories::UserDirs::new() {
        let base = user_dirs
            .document_dir()
            .map(PathBuf::from)
            .unwrap_or_else(|| user_dirs.home_dir().to_path_buf());
        return base.join("OpenDesk Workareas");
    }

    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("OpenDesk Workareas")
}
