mod checks;
mod live;
mod output;
#[cfg(test)]
mod tests;

use crate::{
    app_state::OPENCODE_BIN_SETTING,
    db::{self, settings},
    opencode_client::binary::{OpenCodeBinaryInfo, OpenCodeLocator},
    paths::AppPaths,
};
use checks::{probe_daemon, probe_shared_services};
use live::probe_live;
use output::print_human;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct DoctorReport {
    pub version: String,
    pub data_dir: String,
    pub config_dir: String,
    pub log_dir: String,
    pub database_ok: bool,
    pub workspaces: i64,
    pub mcp_control_workspaces: i64,
    pub base_pack_workspaces: i64,
    pub binary: Option<OpenCodeBinaryInfo>,
    pub binary_error: Option<String>,
    pub daemon: Option<ServiceProbe>,
    pub shared_services: Vec<ServiceProbe>,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<Vec<LiveProbe>>,
}

#[derive(Debug, Serialize)]
pub struct LiveProbe {
    pub name: String,
    pub status: String,
    pub detail: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ServiceProbe {
    pub source: String,
    pub url: Option<String>,
    pub version: Option<String>,
    pub online: bool,
    pub error: Option<String>,
}

pub fn run(json: bool, fix_daemons: bool, live: bool) -> i32 {
    if fix_daemons {
        crate::opencode_client::daemon_manager::cleanup_orphaned_daemons();
    }
    let report = match build_report(live) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("OpenDesk doctor failed: {error:#}");
            return 2;
        }
    };

    if json {
        match serde_json::to_string_pretty(&report) {
            Ok(text) => println!("{text}"),
            Err(error) => {
                eprintln!("could not encode the report: {error}");
                return 2;
            }
        }
    } else {
        print_human(&report);
    }

    if report.binary.is_some()
        || report
            .daemon
            .as_ref()
            .map(|probe| probe.online)
            .unwrap_or(false)
    {
        0
    } else {
        1
    }
}

fn build_report(live: bool) -> anyhow::Result<DoctorReport> {
    let paths = AppPaths::discover()?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let mut warnings = Vec::new();
        let mut live_pool: Option<sqlx::SqlitePool> = None;
        let (database_ok, workspaces, mcp_control_workspaces, base_pack_workspaces, configured_binary) =
            match db::init(&paths.db_path).await {
                Ok(pool) => {
                    live_pool = Some(pool.clone());
                    let workspaces: i64 =
                        sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
                            .fetch_one(&pool)
                            .await
                            .unwrap_or(0);
                    let list = crate::db::workspaces::list(&pool)
                        .await
                        .unwrap_or_default();
                    let mut mcp_control_workspaces = 0;
                    let mut base_pack_workspaces = 0;
                    for workspace in &list {
                        let root = Path::new(&workspace.root_path);
                        let config = std::fs::read_to_string(root.join("opencode.jsonc"))
                            .unwrap_or_default();
                        if config.contains("\"opendesk-control\"") {
                            mcp_control_workspaces += 1;
                        } else {
                            warnings.push(format!(
                                "workspace '{}' sem MCP opendesk-control no opencode.jsonc",
                                workspace.name
                            ));
                        }
                        if root
                            .join(".opencode/skills/operacao-opendesk/SKILL.md")
                            .is_file()
                        {
                            base_pack_workspaces += 1;
                        } else {
                            warnings.push(format!(
                                "workspace '{}' sem o pacote base controle-opendesk; instale em Pacotes se quiser a skill e os comandos",
                                workspace.name
                            ));
                        }
                    }
                    let configured = settings::get(&pool, OPENCODE_BIN_SETTING)
                        .await
                        .ok()
                        .flatten()
                        .and_then(|value| value.as_str().map(PathBuf::from));
                    (true, workspaces, mcp_control_workspaces, base_pack_workspaces, configured)
                }
                Err(error) => {
                    warnings.push(format!("could not open the database: {error:#}"));
                    (false, 0, 0, 0, None)
                }
            };

        let locator = OpenCodeLocator::new(configured_binary);
        let (binary, binary_error) = match locator.resolve() {
            Ok(binary) => (Some(binary), None),
            Err(error) => (None, Some(format!("{error:#}"))),
        };
        if !database_ok {
            warnings.push("SQLite state is unavailable; runs, queue and settings will not persist".into());
        }
        if binary.is_none() {
            warnings.push(
                "OpenCode V2 was not found. Install it or set the binary path in Settings → OpenCode Engine."
                    .into(),
            );
        }
        if !paths.log_dir.is_dir() {
            warnings.push(format!(
                "log directory is missing: {}",
                paths.log_dir.display()
            ));
        }

        let daemon = probe_daemon(&paths.daemon_lock_path).await;
        let shared_services = probe_shared_services().await;
        let live_report = if live {
            match live_pool.as_ref() {
                Some(pool) => Some(probe_live(pool, &paths, &daemon).await),
                None => Some(vec![LiveProbe {
                    name: "database".into(),
                    status: "unavailable".into(),
                    detail: None,
                }]),
            }
        } else {
            None
        };
        match &daemon {
            Some(probe) if probe.online => {}
            _ if shared_services.is_empty() && binary.is_none() => warnings.push(
                "no dedicated daemon, no shared OpenCode service and no binary to start one".into(),
            ),
            _ => {}
        }

        Ok(DoctorReport {
            version: env!("CARGO_PKG_VERSION").into(),
            data_dir: paths.data_dir.to_string_lossy().to_string(),
            config_dir: paths.config_dir.to_string_lossy().to_string(),
            log_dir: paths.log_dir.to_string_lossy().to_string(),
            database_ok,
            workspaces,
            mcp_control_workspaces,
            base_pack_workspaces,
            binary,
            binary_error,
            daemon,
            shared_services,
            warnings,
            live: live_report,
        })
    })
}
