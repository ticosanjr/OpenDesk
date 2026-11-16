//! Runtime harnesses beyond OpenCode.
//!
//! OpenDesk owns orchestration (queue, budget, HITL, artifacts). The harness is
//! interchangeable: OpenCode V2 through the local daemon, or a CLI agent
//! (Claude Code, Codex, Pi, Hermes) spawned inside the workspace. Each adapter
//! exposes the same [`crate::runtime::AgentRuntimeAdapter`] contract and emits
//! normalized [`crate::runtime::RuntimeEvent`]s.

pub mod cli;
pub mod execution;
pub mod opencode;

use crate::{
    db::settings,
    error::{AppError, AppResult},
    opencode_client::daemon_manager::DaemonManager,
    runtime::AgentRuntimeAdapter,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::Duration,
};

/// Built-in harness ids. Custom CLI adapters can be added to the DB later; the
/// built-ins are always valid task/workspace choices.
pub const BUILTIN_HARNESSES: &[&str] = &["opencode", "claude-code", "codex", "pi", "hermes"];
pub const DEFAULT_HARNESS_SETTING: &str = "default_harness";

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct HarnessSpec {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub command: Option<String>,
    pub capabilities_json: String,
    pub status: String,
    pub args_json: String,
    pub health_args_json: String,
    #[sqlx(default)]
    pub last_health_at: Option<String>,
}

impl HarnessSpec {
    pub fn args(&self) -> Vec<String> {
        serde_json::from_str(&self.args_json).unwrap_or_default()
    }

    pub fn health_args(&self) -> Vec<String> {
        serde_json::from_str(&self.health_args_json).unwrap_or_default()
    }

    pub fn capabilities(&self) -> crate::runtime::RuntimeCapabilities {
        serde_json::from_str(&self.capabilities_json).unwrap_or_default()
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateHarnessInput {
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub health_args: Option<Vec<String>>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HarnessHealth {
    pub id: String,
    pub status: String,
    pub version: Option<String>,
    pub error: Option<String>,
}

/// In-memory view of `adapter_registry`. The DB stays the source of truth so
/// services survive restarts and the settings UI can edit commands.
pub struct AdapterService {
    specs: RwLock<HashMap<String, HarnessSpec>>,
}

impl AdapterService {
    pub async fn load(pool: &SqlitePool) -> AppResult<Self> {
        let rows = sqlx::query_as::<_, HarnessSpec>(
            "SELECT id, name, kind, command, capabilities_json, status, args_json,
                    health_args_json, last_health_at
             FROM adapter_registry ORDER BY id",
        )
        .fetch_all(pool)
        .await?;
        let specs = rows
            .into_iter()
            .map(|spec| (spec.id.clone(), spec))
            .collect::<HashMap<_, _>>();
        Ok(Self {
            specs: RwLock::new(specs),
        })
    }

    pub fn list(&self) -> Vec<HarnessSpec> {
        let mut list = self
            .specs
            .read()
            .map(|specs| specs.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        list.sort_by(|left, right| left.id.cmp(&right.id));
        list
    }

    pub fn get(&self, id: &str) -> Option<HarnessSpec> {
        self.specs
            .read()
            .ok()
            .and_then(|specs| specs.get(id).cloned())
    }

    pub async fn update(
        &self,
        pool: &SqlitePool,
        id: &str,
        input: UpdateHarnessInput,
    ) -> AppResult<HarnessSpec> {
        let current = self
            .get(id)
            .ok_or_else(|| AppError::NotFound(format!("harness {id}")))?;
        let command = match input.command {
            Some(value) => {
                let value = value.trim();
                if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                }
            }
            None => current.command.clone(),
        };
        if current.kind == "cli" && command.is_none() {
            return Err(AppError::BadRequest(
                "harness CLI precisa de um comando".into(),
            ));
        }
        let args_json = match input.args {
            Some(args) => serde_json::to_string(&args)
                .map_err(|error| AppError::Internal(error.to_string()))?,
            None => current.args_json.clone(),
        };
        let health_args_json = match input.health_args {
            Some(args) => serde_json::to_string(&args)
                .map_err(|error| AppError::Internal(error.to_string()))?,
            None => current.health_args_json.clone(),
        };
        let name = input.name.unwrap_or_else(|| current.name.clone());
        sqlx::query(
            "UPDATE adapter_registry SET command = ?, args_json = ?, health_args_json = ?,
                    name = ?, status = 'UNKNOWN', last_health_at = NULL,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?",
        )
        .bind(command.as_deref())
        .bind(&args_json)
        .bind(&health_args_json)
        .bind(&name)
        .bind(id)
        .execute(pool)
        .await?;
        let mut updated = current;
        updated.command = command;
        updated.args_json = args_json;
        updated.health_args_json = health_args_json;
        updated.name = name;
        updated.status = "UNKNOWN".into();
        updated.last_health_at = None;
        if let Ok(mut specs) = self.specs.write() {
            specs.insert(id.to_string(), updated.clone());
        }
        Ok(updated)
    }

    /// Resolves the concrete adapter for a job. OpenCode delegates to the
    /// daemon; CLI harnesses are constructed from the persisted spec.
    pub fn runtime(
        &self,
        id: &str,
        daemon: Arc<DaemonManager>,
    ) -> AppResult<Arc<dyn AgentRuntimeAdapter>> {
        let spec = self
            .get(id)
            .ok_or_else(|| AppError::NotFound(format!("harness {id}")))?;
        match spec.kind.as_str() {
            "opencode" => Ok(Arc::new(opencode::OpenCodeAdapter::new(daemon))),
            "cli" => Ok(Arc::new(cli::CliAdapter::new(spec)?)),
            other => Err(AppError::BadRequest(format!(
                "tipo de harness desconhecido: {other}"
            ))),
        }
    }

    /// Checks the harness executable and persists the resulting status.
    pub async fn health(&self, pool: &SqlitePool, id: &str) -> AppResult<HarnessHealth> {
        let spec = self
            .get(id)
            .ok_or_else(|| AppError::NotFound(format!("harness {id}")))?;
        let health = if spec.kind == "opencode" {
            HarnessHealth {
                id: spec.id.clone(),
                status: "READY".into(),
                version: None,
                error: None,
            }
        } else {
            check_cli(&spec).await
        };
        sqlx::query(
            "UPDATE adapter_registry SET status = ?, last_health_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
             WHERE id = ?",
        )
        .bind(&health.status)
        .bind(id)
        .execute(pool)
        .await?;
        if let Ok(mut specs) = self.specs.write()
            && let Some(current) = specs.get_mut(id)
        {
            current.status = health.status.clone();
        }
        Ok(health)
    }
}

async fn check_cli(spec: &HarnessSpec) -> HarnessHealth {
    let Some(command) = spec
        .command
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        return HarnessHealth {
            id: spec.id.clone(),
            status: "MISSING".into(),
            version: None,
            error: Some("comando não configurado".into()),
        };
    };
    let args = if spec.health_args().is_empty() {
        vec!["--version".to_string()]
    } else {
        spec.health_args()
    };
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::process::Command::new(command).args(&args).output(),
    )
    .await;
    match output {
        Ok(Ok(output)) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout)
                .lines()
                .chain(String::from_utf8_lossy(&output.stderr).lines())
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(|line| line.chars().take(160).collect::<String>());
            HarnessHealth {
                id: spec.id.clone(),
                status: "READY".into(),
                version,
                error: None,
            }
        }
        Ok(Ok(output)) => HarnessHealth {
            id: spec.id.clone(),
            status: "ERROR".into(),
            version: None,
            error: Some(format!(
                "{command} saiu com {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
                    .lines()
                    .next()
                    .unwrap_or("")
            )),
        },
        Ok(Err(error)) => HarnessHealth {
            id: spec.id.clone(),
            status: "MISSING".into(),
            version: None,
            error: Some(error.to_string()),
        },
        Err(_) => HarnessHealth {
            id: spec.id.clone(),
            status: "ERROR".into(),
            version: None,
            error: Some("timeout ao verificar o harness".into()),
        },
    }
}

/// Workspace default harness, falling back to OpenCode for anything unknown.
pub async fn default_harness(pool: &SqlitePool) -> AppResult<String> {
    let value = settings::get(pool, DEFAULT_HARNESS_SETTING)
        .await?
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "opencode".into());
    let value = value.trim();
    Ok(if BUILTIN_HARNESSES.contains(&value) {
        value.to_string()
    } else {
        "opencode".into()
    })
}

pub async fn set_default_harness(pool: &SqlitePool, value: &str) -> AppResult<String> {
    let value = validate_harness(value)?;
    settings::set(pool, DEFAULT_HARNESS_SETTING, &serde_json::json!(value)).await?;
    Ok(value)
}

pub fn validate_harness(value: &str) -> AppResult<String> {
    let value = value.trim();
    if BUILTIN_HARNESSES.contains(&value) {
        Ok(value.to_string())
    } else {
        Err(AppError::BadRequest(format!(
            "harness inválido '{value}'; use {}",
            BUILTIN_HARNESSES.join(", ")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn seeds_builtin_harnesses_and_resolves_default() {
        let ctx = crate::test_support::context().await;
        let service = AdapterService::load(&ctx.db).await.expect("load");
        let ids = service
            .list()
            .into_iter()
            .map(|spec| spec.id)
            .collect::<Vec<_>>();
        for id in BUILTIN_HARNESSES {
            assert!(ids.contains(&(*id).to_string()), "missing {id}");
        }
        assert_eq!(default_harness(&ctx.db).await.expect("default"), "opencode");
        set_default_harness(&ctx.db, "codex")
            .await
            .expect("set default");
        assert_eq!(default_harness(&ctx.db).await.expect("default"), "codex");
    }

    #[tokio::test]
    async fn rejects_unknown_harness_names() {
        assert!(validate_harness("nope").is_err());
        assert!(
            set_default_harness(&crate::test_support::context().await.db, "nope")
                .await
                .is_err()
        );
    }
}
