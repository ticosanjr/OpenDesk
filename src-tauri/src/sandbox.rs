//! Execution sandbox policy for CLI harnesses.
//!
//! Real kernel isolation is platform specific. OpenDesk offers three honest
//! modes: `off`, `workspace` (cwd + env scrubbing + command allowlist) and
//! `strict` (bubblewrap with a read-only system and only the workspace writable).
//! `strict` never silently degrades: if `bwrap` is missing the run fails with a
//! clear message.

use crate::{
    db::settings,
    error::{AppError, AppResult},
};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

pub const SANDBOX_SETTING: &str = "sandbox_policy";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SandboxPolicy {
    /// `off`, `workspace` or `strict`.
    pub mode: String,
    /// Keep network access in `strict` mode.
    pub network: bool,
    /// Extra paths mounted read-write in `strict` mode.
    pub allowed_paths: Vec<String>,
    /// Optional command allowlist (basename).
    pub allowed_commands: Vec<String>,
    /// Minutes before the run is interrupted. 0 keeps the default 30.
    pub max_runtime_minutes: u32,
    /// Remove common cloud credentials from the child environment.
    pub scrub_env: bool,
}

impl Default for SandboxPolicy {
    fn default() -> Self {
        Self {
            mode: "off".into(),
            network: true,
            allowed_paths: Vec::new(),
            allowed_commands: Vec::new(),
            max_runtime_minutes: 0,
            scrub_env: true,
        }
    }
}

impl SandboxPolicy {
    pub fn validate(mut self) -> AppResult<Self> {
        self.mode = self.mode.trim().to_lowercase();
        if !matches!(self.mode.as_str(), "off" | "workspace" | "strict") {
            return Err(AppError::BadRequest(format!(
                "modo de sandbox inválido '{}'; use off, workspace ou strict",
                self.mode
            )));
        }
        self.allowed_paths = self
            .allowed_paths
            .into_iter()
            .map(|path| path.trim().to_string())
            .filter(|path| !path.is_empty())
            .collect();
        self.allowed_commands = self
            .allowed_commands
            .into_iter()
            .map(|command| command.trim().to_string())
            .filter(|command| !command.is_empty())
            .collect();
        self.max_runtime_minutes = self.max_runtime_minutes.min(24 * 60);
        Ok(self)
    }

    pub fn command_allowed(&self, command: &str) -> bool {
        if self.allowed_commands.is_empty() {
            return true;
        }
        let base = std::path::Path::new(command)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| command.to_string());
        self.allowed_commands
            .iter()
            .any(|allowed| allowed == &base || allowed == command)
    }

    /// Env vars removed from the child process in `workspace`/`strict`.
    pub fn scrubbed(&self) -> bool {
        self.scrub_env && self.mode != "off"
    }
}

const SENSITIVE_ENV: &[&str] = &[
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AZURE_CLIENT_SECRET",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "DATABASE_URL",
    "OPENDESK_API_TOKEN",
];

pub fn scrub_environment(command: &mut tokio::process::Command, policy: &SandboxPolicy) {
    if !policy.scrubbed() {
        return;
    }
    for key in SENSITIVE_ENV {
        command.env_remove(key);
    }
}

/// Wraps the command with bubblewrap in strict mode. Returns a descriptive error
/// instead of running unsandboxed.
pub fn wrap_strict(
    policy: &SandboxPolicy,
    workspace_root: &str,
    command: &str,
    args: &[String],
) -> AppResult<Vec<String>> {
    if policy.mode != "strict" {
        let mut out = vec![command.to_string()];
        out.extend(args.iter().cloned());
        return Ok(out);
    }
    if !bwrap_available() {
        return Err(AppError::Conflict(
            "sandbox strict exige bubblewrap (bwrap) instalado".into(),
        ));
    }
    let mut wrapped = vec![
        "bwrap".to_string(),
        "--die-with-parent".into(),
        "--new-session".into(),
        "--unshare-pid".into(),
        "--proc".into(),
        "/proc".into(),
        "--dev".into(),
        "/dev".into(),
        "--tmpfs".into(),
        "/tmp".into(),
    ];
    for path in ["/usr", "/bin", "/sbin", "/lib", "/lib64", "/etc"] {
        if std::path::Path::new(path).exists() {
            wrapped.push("--ro-bind".into());
            wrapped.push(path.into());
            wrapped.push(path.into());
        }
    }
    wrapped.push("--bind".into());
    wrapped.push(workspace_root.into());
    wrapped.push(workspace_root.into());
    for path in &policy.allowed_paths {
        if std::path::Path::new(path).exists() {
            wrapped.push("--bind".into());
            wrapped.push(path.clone());
            wrapped.push(path.clone());
        }
    }
    if !policy.network {
        wrapped.push("--unshare-net".into());
    }
    wrapped.push("--chdir".into());
    wrapped.push(workspace_root.into());
    wrapped.push("--".into());
    wrapped.push(command.to_string());
    wrapped.extend(args.iter().cloned());
    Ok(wrapped)
}

fn bwrap_available() -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join("bwrap").is_file()))
        .unwrap_or(false)
}

fn workspace_key(workspace_id: &str) -> String {
    format!("{SANDBOX_SETTING}:{workspace_id}")
}

/// Workspace-specific policy wins over the global default.
pub async fn get_for(pool: &SqlitePool, workspace_id: Option<&str>) -> AppResult<SandboxPolicy> {
    if let Some(workspace_id) = workspace_id
        && let Some(value) = settings::get(pool, &workspace_key(workspace_id)).await?
        && let Ok(policy) = serde_json::from_value::<SandboxPolicy>(value)
    {
        return Ok(policy);
    }
    get(pool).await
}

pub async fn set_for(
    pool: &SqlitePool,
    workspace_id: Option<&str>,
    policy: SandboxPolicy,
) -> AppResult<SandboxPolicy> {
    let policy = policy.validate()?;
    let key = workspace_id
        .map(workspace_key)
        .unwrap_or_else(|| SANDBOX_SETTING.to_string());
    settings::set(
        pool,
        &key,
        &serde_json::to_value(&policy).map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .await?;
    Ok(policy)
}

pub async fn get(pool: &SqlitePool) -> AppResult<SandboxPolicy> {
    let value = settings::get(pool, SANDBOX_SETTING).await?;
    Ok(value
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default())
}

#[derive(Debug, Serialize)]
pub struct SandboxStatus {
    pub policy: SandboxPolicy,
    pub scope: String,
    pub workspace_id: Option<String>,
    pub bwrap_available: bool,
    pub platform: String,
}

pub async fn status(pool: &SqlitePool, workspace_id: Option<&str>) -> AppResult<SandboxStatus> {
    let has_override = if let Some(workspace_id) = workspace_id {
        settings::get(pool, &workspace_key(workspace_id))
            .await?
            .is_some()
    } else {
        false
    };
    Ok(SandboxStatus {
        policy: get_for(pool, workspace_id).await?,
        scope: if has_override { "workspace" } else { "global" }.into(),
        workspace_id: workspace_id.map(str::to_string),
        bwrap_available: bwrap_available(),
        platform: std::env::consts::OS.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_mode_and_command_allowlist() {
        let policy = SandboxPolicy {
            mode: "STRICT".into(),
            allowed_commands: vec!["claude".into()],
            ..Default::default()
        }
        .validate()
        .expect("valid");
        assert_eq!(policy.mode, "strict");
        assert!(policy.command_allowed("claude"));
        assert!(policy.command_allowed("/usr/local/bin/claude"));
        assert!(!policy.command_allowed("rm"));

        assert!(
            SandboxPolicy {
                mode: "root".into(),
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn workspace_mode_keeps_command_unwrapped() {
        let policy = SandboxPolicy {
            mode: "workspace".into(),
            ..Default::default()
        };
        let args = wrap_strict(&policy, "/tmp/ws", "claude", &["-p".into()]).expect("wrap");
        assert_eq!(args, vec!["claude", "-p"]);
    }

    #[tokio::test]
    async fn persists_the_policy() {
        let ctx = crate::test_support::context().await;
        let policy = set_for(
            &ctx.state.db,
            None,
            SandboxPolicy {
                mode: "workspace".into(),
                scrub_env: true,
                ..Default::default()
            },
        )
        .await
        .expect("set");
        assert_eq!(policy.mode, "workspace");
        assert_eq!(get(&ctx.state.db).await.expect("get").mode, "workspace");

        set_for(
            &ctx.state.db,
            Some(&ctx.workspace_id),
            SandboxPolicy {
                mode: "strict".into(),
                ..Default::default()
            },
        )
        .await
        .expect("workspace set");
        assert_eq!(
            get_for(&ctx.state.db, Some(&ctx.workspace_id))
                .await
                .expect("workspace get")
                .mode,
            "strict"
        );
        assert_eq!(
            get_for(&ctx.state.db, None).await.expect("global").mode,
            "workspace"
        );
    }
}
