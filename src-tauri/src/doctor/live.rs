use crate::{db::settings, paths::AppPaths};
use serde_json::Value;

use super::{LiveProbe, ServiceProbe};

pub(super) async fn probe_live(
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

    let quick_check = sqlx::query_scalar::<_, String>("PRAGMA quick_check")
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| format!("error: {error}"));
    let journal = sqlx::query_scalar::<_, String>("PRAGMA journal_mode")
        .fetch_one(pool)
        .await
        .unwrap_or_else(|error| format!("error: {error}"));
    probes.push(LiveProbe {
        name: "sqlite".into(),
        status: if quick_check.eq_ignore_ascii_case("ok") && journal.eq_ignore_ascii_case("wal") {
            "ok"
        } else {
            "warn"
        }
        .into(),
        detail: Some(format!("quick_check={quick_check}; journal_mode={journal}")),
    });

    let sandbox_tools = ["bwrap", "docker", "podman"];
    let found: Vec<&str> = sandbox_tools
        .into_iter()
        .filter(|tool| {
            std::process::Command::new("which")
                .arg(tool)
                .output()
                .map(|output| output.status.success())
                .unwrap_or(false)
        })
        .collect();
    probes.push(LiveProbe {
        name: "sandbox".into(),
        status: if found.is_empty() {
            "not-configured"
        } else {
            "ok"
        }
        .into(),
        detail: if found.is_empty() {
            Some("bwrap/docker/podman ausentes; use sandbox workspace/off".into())
        } else {
            Some(format!("disponível: {}", found.join(", ")))
        },
    });

    let display = std::env::var("DISPLAY").ok();
    let display_ready = display
        .as_deref()
        .map(|value| {
            std::path::Path::new(&format!(
                "/tmp/.X11-unix/X{}",
                value.trim_start_matches(':')
            ))
            .exists()
        })
        .unwrap_or(false);
    probes.push(LiveProbe {
        name: "display".into(),
        status: if display_ready {
            "ok"
        } else {
            "not-configured"
        }
        .into(),
        detail: display,
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

pub(super) async fn probe_telegram(token: Option<String>, enabled: Option<bool>) -> LiveProbe {
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

pub(super) async fn probe_signal(
    config: Option<crate::integrations::signal::SignalConfig>,
) -> LiveProbe {
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

pub(super) async fn probe_whatsapp(config: Option<Value>, token: Option<String>) -> LiveProbe {
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
