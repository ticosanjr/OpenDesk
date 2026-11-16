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

async fn probe_live(
    pool: &sqlx::SqlitePool,
    paths: &AppPaths,
    daemon: &Option<ServiceProbe>,
) -> Vec<LiveProbe> {
    let secrets = std::sync::Arc::new(crate::secrets::SecretStore::new(paths));
    let workspace_id = settings::active_workspace_id(pool).await.ok().flatten();
    let mut probes = Vec::new();

    probes.push(LiveProbe {
        name: "opencode-daemon".into(),
        status: if daemon.as_ref().is_some_and(|probe| probe.online) {
            "ok".into()
        } else {
            "offline".into()
        },
        detail: daemon.as_ref().and_then(|probe| probe.error.clone()),
    });

    let adapters = crate::adapters::AdapterService::load(pool).await;
    match adapters {
        Ok(service) => {
            for spec in service.list().into_iter().filter(|spec| spec.kind == "cli") {
                let health = service.health(pool, &spec.id).await;
                probes.push(LiveProbe {
                    name: format!("harness:{}", spec.id),
                    status: health
                        .as_ref()
                        .map(|health| health.status.to_lowercase())
                        .unwrap_or_else(|_| "error".into()),
                    detail: health
                        .ok()
                        .and_then(|health| health.version.or(health.error)),
                });
            }
        }
        Err(error) => probes.push(LiveProbe {
            name: "harnesses".into(),
            status: "error".into(),
            detail: Some(error.to_string()),
        }),
    }

    if let Some(workspace_id) = workspace_id {
        match crate::integrations::smtp_mailer::load_config(pool, &secrets, &workspace_id).await {
            Ok(Some(config)) => {
                let result = crate::integrations::smtp_mailer::test_connection(&config).await;
                probes.push(LiveProbe {
                    name: "smtp".into(),
                    status: if result.is_ok() { "ok" } else { "error" }.into(),
                    detail: result.err().map(|error| format!("{error:#}")),
                });
            }
            Ok(None) => probes.push(LiveProbe {
                name: "smtp".into(),
                status: "not-configured".into(),
                detail: None,
            }),
            Err(error) => probes.push(LiveProbe {
                name: "smtp".into(),
                status: "error".into(),
                detail: Some(format!("{error:#}")),
            }),
        }

        let google = crate::integrations::google_oauth::GoogleOAuth::new(
            pool.clone(),
            secrets.clone(),
            crate::events::EventBus::new(8),
        );
        probes.push(match google.status(&workspace_id).await {
            Ok(status) if status.connected => match google.access_token(&workspace_id).await {
                Ok(_) => LiveProbe {
                    name: "google".into(),
                    status: "ok".into(),
                    detail: status.email,
                },
                Err(error) => LiveProbe {
                    name: "google".into(),
                    status: "error".into(),
                    detail: Some(format!("{error:#}")),
                },
            },
            Ok(_) => LiveProbe {
                name: "google".into(),
                status: "not-configured".into(),
                detail: None,
            },
            Err(error) => LiveProbe {
                name: "google".into(),
                status: "error".into(),
                detail: Some(format!("{error:#}")),
            },
        });
    }

    let telegram = crate::integrations::telegram::load(pool).await.ok();
    let telegram_token = crate::integrations::telegram::token(&secrets);
    probes.push(
        probe_telegram(
            telegram_token,
            telegram.as_ref().map(|config| config.enabled),
        )
        .await,
    );

    let signal = crate::integrations::signal::load(pool).await.ok();
    probes.push(probe_signal(signal).await);

    let whatsapp = crate::db::settings::get(pool, "channels.whatsapp")
        .await
        .ok()
        .flatten();
    let whatsapp_token = secrets
        .get("channel_whatsapp", "access_token")
        .ok()
        .flatten();
    probes.push(probe_whatsapp(whatsapp, whatsapp_token).await);

    let mtls = crate::mtls::from_env();
    probes.push(LiveProbe {
        name: "mtls".into(),
        status: if mtls.configured {
            if mtls.valid { "ok" } else { "error" }
        } else {
            "not-configured"
        }
        .into(),
        detail: mtls.detail,
    });

    let origin = std::env::var("OPENDESK_WEBAUTHN_ORIGIN").ok();
    probes.push(LiveProbe {
        name: "webauthn".into(),
        status: if origin.is_some() {
            "configured"
        } else {
            "origin-from-request"
        }
        .into(),
        detail: origin,
    });

    probes
}

async fn probe_telegram(token: Option<String>, enabled: Option<bool>) -> LiveProbe {
    let Some(token) = token.filter(|value| !value.trim().is_empty()) else {
        return LiveProbe {
            name: "telegram".into(),
            status: "not-configured".into(),
            detail: None,
        };
    };
    if !enabled.unwrap_or(false) {
        return LiveProbe {
            name: "telegram".into(),
            status: "disabled".into(),
            detail: Some("token presente, bot desabilitado".into()),
        };
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .unwrap_or_default();
    match client
        .get(format!("https://api.telegram.org/bot{token}/getMe"))
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            let body: Value = response.json().await.unwrap_or(Value::Null);
            let username = body
                .pointer("/result/username")
                .and_then(Value::as_str)
                .map(|value| format!("@{value}"))
                .unwrap_or_else(|| "bot".into());
            LiveProbe {
                name: "telegram".into(),
                status: "ok".into(),
                detail: Some(username),
            }
        }
        Ok(response) => LiveProbe {
            name: "telegram".into(),
            status: "error".into(),
            detail: Some(format!("getMe HTTP {}", response.status().as_u16())),
        },
        Err(error) => LiveProbe {
            name: "telegram".into(),
            status: "error".into(),
            detail: Some(format!("{error:#}")),
        },
    }
}

async fn probe_signal(config: Option<crate::integrations::signal::SignalConfig>) -> LiveProbe {
    let Some(config) = config else {
        return LiveProbe {
            name: "signal".into(),
            status: "not-configured".into(),
            detail: None,
        };
    };
    if config.account.trim().is_empty() {
        return LiveProbe {
            name: "signal".into(),
            status: "not-configured".into(),
            detail: Some(config.cli_path),
        };
    }
    match crate::integrations::signal::detect_cli(&config.cli_path).await {
        Some(version) => LiveProbe {
            name: "signal".into(),
            status: "ok".into(),
            detail: Some(format!("{} · {version}", config.account)),
        },
        None => LiveProbe {
            name: "signal".into(),
            status: "error".into(),
            detail: Some(format!("signal-cli não respondeu em {}", config.cli_path)),
        },
    }
}

async fn probe_whatsapp(config: Option<Value>, token: Option<String>) -> LiveProbe {
    let Some(config) = config else {
        return LiveProbe {
            name: "whatsapp".into(),
            status: "not-configured".into(),
            detail: None,
        };
    };
    let provider = config
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("structure");
    if config.get("enabled").and_then(Value::as_bool) != Some(true) {
        return LiveProbe {
            name: "whatsapp".into(),
            status: "disabled".into(),
            detail: Some(format!("provider: {provider}")),
        };
    }
    if provider != "cloud-api" {
        return LiveProbe {
            name: "whatsapp".into(),
            status: "configured".into(),
            detail: Some(format!("provider: {provider} (sem probe de rede)")),
        };
    }
    let Some(token) = token else {
        return LiveProbe {
            name: "whatsapp".into(),
            status: "error".into(),
            detail: Some("access_token ausente".into()),
        };
    };
    let phone_number_id = config
        .get("phone_number_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let version = config
        .get("api_version")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("v21.0");
    if phone_number_id.trim().is_empty() {
        return LiveProbe {
            name: "whatsapp".into(),
            status: "error".into(),
            detail: Some("phone_number_id ausente".into()),
        };
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(6))
        .build()
        .unwrap_or_default();
    let url = format!(
        "https://graph.facebook.com/{version}/{phone_number_id}?fields=display_phone_number,verified_name&access_token={token}"
    );
    match client.get(url).send().await {
        Ok(response) if response.status().is_success() => {
            let body: Value = response.json().await.unwrap_or(Value::Null);
            LiveProbe {
                name: "whatsapp".into(),
                status: "ok".into(),
                detail: body
                    .get("display_phone_number")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .or_else(|| {
                        body.get("verified_name")
                            .and_then(Value::as_str)
                            .map(str::to_string)
                    }),
            }
        }
        Ok(response) => LiveProbe {
            name: "whatsapp".into(),
            status: "error".into(),
            detail: Some(format!("Graph HTTP {}", response.status().as_u16())),
        },
        Err(error) => LiveProbe {
            name: "whatsapp".into(),
            status: "error".into(),
            detail: Some(format!("{error:#}")),
        },
    }
}

fn print_human(report: &DoctorReport) {
    println!("OpenDesk {} — doctor\n", report.version);
    println!("data:    {}", report.data_dir);
    println!("config:  {}", report.config_dir);
    println!("logs:    {}", report.log_dir);
    println!(
        "MCP ctl: {}/{} workspaces · pacote base: {}/{}",
        report.mcp_control_workspaces,
        report.workspaces,
        report.base_pack_workspaces,
        report.workspaces
    );
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

    #[tokio::test]
    async fn live_probes_degrade_to_not_configured() {
        assert_eq!(probe_telegram(None, None).await.status, "not-configured");
        assert_eq!(probe_signal(None).await.status, "not-configured");
        assert_eq!(probe_whatsapp(None, None).await.status, "not-configured");
    }
}
