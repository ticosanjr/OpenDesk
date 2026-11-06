use crate::{
    app_state::OPENCODE_BIN_SETTING,
    db::{self, settings},
    opencode_client::{
        binary::{OpenCodeBinaryInfo, OpenCodeLocator},
        rest_client::RestClient,
    },
    paths::AppPaths,
};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
pub struct DoctorReport {
    pub version: String,
    pub data_dir: String,
    pub config_dir: String,
    pub log_dir: String,
    pub database_ok: bool,
    pub workspaces: i64,
    pub binary: Option<OpenCodeBinaryInfo>,
    pub binary_error: Option<String>,
    pub daemon: Option<ServiceProbe>,
    pub shared_services: Vec<ServiceProbe>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ServiceProbe {
    pub source: String,
    pub url: Option<String>,
    pub version: Option<String>,
    pub online: bool,
    pub error: Option<String>,
}

pub fn run(json: bool) -> i32 {
    let report = match build_report() {
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

fn build_report() -> anyhow::Result<DoctorReport> {
    let paths = AppPaths::discover()?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async move {
        let mut warnings = Vec::new();
        let (database_ok, workspaces, configured_binary) =
            match db::init(&paths.db_path).await {
                Ok(pool) => {
                    let workspaces: i64 =
                        sqlx::query_scalar("SELECT COUNT(*) FROM workspaces")
                            .fetch_one(&pool)
                            .await
                            .unwrap_or(0);
                    let configured = settings::get(&pool, OPENCODE_BIN_SETTING)
                        .await
                        .ok()
                        .flatten()
                        .and_then(|value| value.as_str().map(PathBuf::from));
                    (true, workspaces, configured)
                }
                Err(error) => {
                    warnings.push(format!("could not open the database: {error:#}"));
                    (false, 0, None)
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
            binary,
            binary_error,
            daemon,
            shared_services,
            warnings,
        })
    })
}

async fn probe_daemon(lock_path: &Path) -> Option<ServiceProbe> {
    let bytes = std::fs::read(lock_path).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    let port = value.get("port").and_then(Value::as_u64).unwrap_or(0) as u16;
    let password = value
        .get("password")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    if port == 0 || password.is_empty() {
        return Some(ServiceProbe {
            source: format!("dedicated ({})", lock_path.display()),
            url: None,
            version: None,
            online: false,
            error: Some("daemon.json is missing port or password".into()),
        });
    }
    let rest = RestClient::new(port, password);
    Some(probe(&rest, format!("dedicated ({})", lock_path.display())).await)
}

async fn probe_shared_services() -> Vec<ServiceProbe> {
    let mut probes = Vec::new();
    for path in service_state_paths() {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        let Some(url) = value.get("url").and_then(Value::as_str) else {
            continue;
        };
        let Some(password) = value.get("password").and_then(Value::as_str) else {
            continue;
        };
        let rest = RestClient::new_with_url(url, password.to_string());
        probes.push(probe(&rest, format!("shared ({})", path.display())).await);
    }
    probes
}

async fn probe(rest: &RestClient, source: String) -> ServiceProbe {
    match rest.info().await {
        Ok(info) => {
            let online = info.version.starts_with('2');
            ServiceProbe {
                source,
                url: Some(rest.base_url().to_string()),
                version: Some(info.version),
                online,
                error: None,
            }
        }
        Err(error) => ServiceProbe {
            source,
            url: Some(rest.base_url().to_string()),
            version: None,
            online: false,
            error: Some(format!("{error:#}")),
        },
    }
}

fn service_state_paths() -> Vec<PathBuf> {
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

fn print_human(report: &DoctorReport) {
    println!("OpenDesk {} — doctor\n", report.version);
    println!("data:    {}", report.data_dir);
    println!("config:  {}", report.config_dir);
    println!("logs:    {}", report.log_dir);
    println!(
        "database: {} ({} workspaces)",
        if report.database_ok {
            "ok"
        } else {
            "unavailable"
        },
        report.workspaces
    );

    match &report.binary {
        Some(binary) => println!(
            "opencode: {} (v{} · método {})",
            binary.path,
            binary.version,
            binary.method.as_deref().unwrap_or("desconhecido")
        ),
        None => println!(
            "opencode: não encontrado — {}",
            report
                .binary_error
                .as_deref()
                .unwrap_or("nenhum caminho testado funcionou")
        ),
    }

    match &report.daemon {
        Some(probe) => println!(
            "daemon: {} — {}",
            probe.source,
            if probe.online {
                format!("online v{}", probe.version.as_deref().unwrap_or("?"))
            } else {
                format!(
                    "offline ({})",
                    probe.error.as_deref().unwrap_or("sem detalhe")
                )
            }
        ),
        None => println!("daemon: sem daemon.json (nenhum daemon dedicado iniciado)"),
    }

    for probe in &report.shared_services {
        println!(
            "serviço: {} — {}",
            probe.source,
            if probe.online {
                format!("online v{}", probe.version.as_deref().unwrap_or("?"))
            } else {
                format!(
                    "offline ({})",
                    probe.error.as_deref().unwrap_or("sem detalhe")
                )
            }
        );
    }

    if report.warnings.is_empty() {
        println!("\nTudo certo.");
    } else {
        println!("\nAvisos:");
        for warning in &report.warnings {
            println!("  - {warning}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_state_paths_target_opencode_service_json() {
        let paths = service_state_paths();
        assert!(!paths.is_empty());
        assert!(
            paths
                .iter()
                .all(|path| path.ends_with("opencode/service.json"))
        );
    }

    #[tokio::test]
    async fn probe_reports_unreachable_services() {
        let client = RestClient::new_with_url("http://127.0.0.1:1", "secret".into());
        let probe = probe(&client, "test".into()).await;
        assert!(!probe.online);
        assert!(probe.error.is_some());
        assert_eq!(probe.source, "test");
    }
}
