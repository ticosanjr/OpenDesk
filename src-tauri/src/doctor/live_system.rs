//! Additional live probes for Vault, HTTP pollers and recipes.

use crate::{
    db::{io_bus as io_bus_db, vault, workspaces},
    doctor::LiveProbe,
    recipes::{parser, store as recipe_store},
};
use chrono::Utc;

pub(crate) async fn probe_vault(pool: &sqlx::SqlitePool) -> LiveProbe {
    let mut seen = std::collections::HashSet::new();
    let mut expired = 0usize;
    let mut without_acl = 0usize;
    let mut total = 0usize;
    for workspace in workspaces::list(pool).await.unwrap_or_default() {
        for entry in vault::list_entries(pool, Some(&workspace.id))
            .await
            .unwrap_or_default()
        {
            if !seen.insert(entry.id.clone()) {
                continue;
            }
            total += 1;
            if entry
                .expires_at
                .as_deref()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .is_some_and(|value| Utc::now() > value.with_timezone(&Utc))
            {
                expired += 1;
            }
            if vault::list_acl(pool, &entry.id)
                .await
                .map(|acls| acls.iter().all(|acl| !acl.can_inject))
                .unwrap_or(true)
            {
                without_acl += 1;
            }
        }
    }
    LiveProbe {
        name: "vault".into(),
        status: if expired > 0 || without_acl > 0 {
            "warning"
        } else {
            "ok"
        }
        .into(),
        detail: Some(format!(
            "{total} entrada(s), {expired} expirada(s), {without_acl} sem ACL de injeção"
        )),
    }
}

pub(crate) async fn probe_http_pollers(pool: &sqlx::SqlitePool) -> LiveProbe {
    let sources = io_bus_db::all_inbound(pool).await.unwrap_or_default();
    let pollers = sources
        .iter()
        .filter(|source| source.source_type == "HTTP_POLLER")
        .collect::<Vec<_>>();
    let failing = pollers
        .iter()
        .filter(|source| source.is_enabled && source.last_status.as_deref() == Some("ERROR"))
        .count();
    LiveProbe {
        name: "http-poller".into(),
        status: if failing > 0 {
            "warning"
        } else if pollers.is_empty() {
            "not-configured"
        } else {
            "ok"
        }
        .into(),
        detail: Some(format!("{} fonte(s), {} com erro", pollers.len(), failing)),
    }
}

pub(crate) async fn probe_recipes(pool: &sqlx::SqlitePool) -> LiveProbe {
    let mut total = 0usize;
    let mut invalid = 0usize;
    for index in recipe_store::list_all(pool).await.unwrap_or_default() {
        if !index.is_enabled {
            continue;
        }
        total += 1;
        match std::fs::read_to_string(&index.file_path) {
            Ok(yaml) if parser::parse(&yaml).is_ok() => {}
            _ => invalid += 1,
        }
    }
    LiveProbe {
        name: "recipes".into(),
        status: if invalid > 0 {
            "warning"
        } else if total == 0 {
            "not-configured"
        } else {
            "ok"
        }
        .into(),
        detail: Some(format!("{total} habilitada(s), {invalid} inválida(s)")),
    }
}

pub(crate) async fn probe_discord(
    config: Option<opendesk_channels::discord::DiscordConfig>,
    token: Option<String>,
) -> LiveProbe {
    let Some(config) = config else {
        return LiveProbe {
            name: "discord".into(),
            status: "not-configured".into(),
            detail: None,
        };
    };
    if !config.enabled {
        return LiveProbe {
            name: "discord".into(),
            status: "disabled".into(),
            detail: None,
        };
    }
    let Some(token) = token else {
        return LiveProbe {
            name: "discord".into(),
            status: "error".into(),
            detail: Some("bot token ausente".into()),
        };
    };
    let response = reqwest::Client::new()
        .get("https://discord.com/api/v10/users/@me")
        .header("authorization", format!("Bot {token}"))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;
    match response {
        Ok(response) if response.status().is_success() => {
            let value: serde_json::Value = response.json().await.unwrap_or_default();
            LiveProbe {
                name: "discord".into(),
                status: "ok".into(),
                detail: value
                    .get("username")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
            }
        }
        Ok(response) => LiveProbe {
            name: "discord".into(),
            status: "error".into(),
            detail: Some(format!("Discord respondeu {}", response.status().as_u16())),
        },
        Err(error) => LiveProbe {
            name: "discord".into(),
            status: "error".into(),
            detail: Some(format!("{error}")),
        },
    }
}

pub(crate) async fn probe_slack(
    config: Option<opendesk_channels::slack::SlackConfig>,
    app_token: Option<String>,
    bot_token: Option<String>,
) -> LiveProbe {
    let Some(config) = config else {
        return LiveProbe {
            name: "slack".into(),
            status: "not-configured".into(),
            detail: None,
        };
    };
    if !config.enabled {
        return LiveProbe {
            name: "slack".into(),
            status: "disabled".into(),
            detail: None,
        };
    }
    if app_token.is_none() || bot_token.is_none() {
        return LiveProbe {
            name: "slack".into(),
            status: "error".into(),
            detail: Some("app/bot token ausente".into()),
        };
    }
    let response = reqwest::Client::new()
        .post("https://slack.com/api/auth.test")
        .bearer_auth(bot_token.unwrap_or_default())
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;
    match response {
        Ok(response) => {
            let value: serde_json::Value = response.json().await.unwrap_or_default();
            if value.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
                LiveProbe {
                    name: "slack".into(),
                    status: "ok".into(),
                    detail: value
                        .get("user")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string),
                }
            } else {
                LiveProbe {
                    name: "slack".into(),
                    status: "error".into(),
                    detail: Some(
                        value
                            .get("error")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("auth.test falhou")
                            .to_string(),
                    ),
                }
            }
        }
        Err(error) => LiveProbe {
            name: "slack".into(),
            status: "error".into(),
            detail: Some(format!("{error}")),
        },
    }
}

pub(crate) fn probe_qr(
    config: Option<opendesk_channels::whatsapp::WhatsAppQrConfig>,
    last_status: Option<String>,
) -> LiveProbe {
    let Some(config) = config else {
        return LiveProbe {
            name: "whatsapp-qr".into(),
            status: "not-configured".into(),
            detail: None,
        };
    };
    if !config.enabled {
        return LiveProbe {
            name: "whatsapp-qr".into(),
            status: "disabled".into(),
            detail: None,
        };
    }
    if config.sidecar_command.trim().is_empty() {
        return LiveProbe {
            name: "whatsapp-qr".into(),
            status: "error".into(),
            detail: Some("sidecar_command ausente".into()),
        };
    }
    let status = last_status.unwrap_or_else(|| "aguardando sidecar".into());
    let warning = status.contains("error") || status.contains("disconnected");
    LiveProbe {
        name: "whatsapp-qr".into(),
        status: if warning { "warning" } else { "ok" }.into(),
        detail: Some(format!(
            "sidecar: {} · status: {status}",
            config.sidecar_command
        )),
    }
}
