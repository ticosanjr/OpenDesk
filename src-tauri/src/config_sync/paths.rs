use crate::error::{AppError, AppResult};
use std::path::{Path, PathBuf};

/// Paths for the native OpenCode configuration. `global_dir` is
/// `~/.config/opencode` on Unix and `%APPDATA%/opencode` on Windows.
#[derive(Clone, Debug)]
pub struct StudioPaths {
    pub global_dir: PathBuf,
    pub workspace_root: Option<PathBuf>,
}

impl StudioPaths {
    pub fn new(workspace_root: Option<PathBuf>) -> Self {
        Self {
            global_dir: opencode_global_dir(),
            workspace_root,
        }
    }

    pub fn base_dir(&self, scope: &str) -> AppResult<PathBuf> {
        match scope {
            "global" => Ok(self.global_dir.clone()),
            "workspace" => self.workspace_root.clone().ok_or_else(|| {
                AppError::BadRequest("a workspace is required for this scope".into())
            }),
            other => Err(AppError::BadRequest(format!(
                "invalid scope '{other}'; expected global or workspace"
            ))),
        }
    }

    pub fn opencode_dir(&self, scope: &str) -> AppResult<PathBuf> {
        match scope {
            "global" => Ok(self.global_dir.clone()),
            "workspace" => Ok(self.base_dir(scope)?.join(".opencode")),
            other => Err(AppError::BadRequest(format!(
                "invalid scope '{other}'; expected global or workspace"
            ))),
        }
    }

    pub fn agents_dir(&self, scope: &str) -> AppResult<PathBuf> {
        Ok(self.opencode_dir(scope)?.join("agents"))
    }

    pub fn skills_dir(&self, scope: &str) -> AppResult<PathBuf> {
        Ok(self.opencode_dir(scope)?.join("skills"))
    }

    pub fn commands_dir(&self, scope: &str) -> AppResult<PathBuf> {
        Ok(self.opencode_dir(scope)?.join("commands"))
    }

    pub fn config_path(&self, scope: &str) -> AppResult<PathBuf> {
        let base = self.base_dir(scope)?;
        let candidates = [
            base.join("opencode.jsonc"),
            base.join("opencode.json"),
            base.join(".opencode/opencode.jsonc"),
            base.join(".opencode/opencode.json"),
        ];
        for candidate in candidates {
            if candidate.exists() {
                return Ok(candidate);
            }
        }
        Ok(base.join("opencode.jsonc"))
    }

    pub fn ensure_scope_layout(&self, scope: &str) -> AppResult<()> {
        for dir in [
            self.agents_dir(scope)?,
            self.skills_dir(scope)?,
            self.commands_dir(scope)?,
        ] {
            std::fs::create_dir_all(&dir).map_err(|error| {
                AppError::Internal(format!("could not create {}: {error}", dir.display()))
            })?;
        }
        Ok(())
    }

    pub fn workspace_output_dir(&self) -> Option<PathBuf> {
        self.workspace_root.as_ref().map(|root| root.join("output"))
    }

    pub fn contains(&self, scope: &str, path: &Path) -> AppResult<bool> {
        let base = self.opencode_dir(scope)?;
        let base = std::fs::canonicalize(&base).unwrap_or(base);
        let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        Ok(path.starts_with(base))
    }
}

pub fn opencode_global_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("opencode");
        }
    }

    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("opencode");
    }

    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".config").join("opencode")
}
