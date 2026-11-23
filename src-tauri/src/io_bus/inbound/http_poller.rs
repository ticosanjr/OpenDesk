//! HTTP/RSS/Atom/JSON/HTML poller.
//!
//! One worker per enabled `HTTP_POLLER` source. The worker stores ETag,
//! Last-Modified and item hashes so repeated polls never duplicate tasks.
//! Network errors update `io_poller_state` and the worker keeps running.

use crate::{
    app_state::AppState,
    db::io_bus::{self, InboundSource},
    error::{AppError, AppResult},
    io_bus::{NormalizedTaskEvent, source_config},
};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Deserialize)]
pub struct PollerConfig {
    pub url: String,
    #[serde(default = "default_format")]
    pub format: String,
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
    #[serde(default)]
    pub selector: Option<String>,
    #[serde(default)]
    pub items_path: Option<String>,
    #[serde(default)]
    pub title_path: Option<String>,
    #[serde(default)]
    pub body_path: Option<String>,
    #[serde(default)]
    pub id_path: Option<String>,
    #[serde(default)]
    pub vault_ref: Option<String>,
    #[serde(default)]
    pub emit_mode: Option<String>,
    #[serde(default)]
    pub max_items: Option<usize>,
    #[serde(default)]
    pub allow_private_network: bool,
}

fn default_format() -> String {
    "rss".into()
}
fn default_interval() -> u64 {
    300
}

#[derive(Debug, Clone)]
struct PollerItem {
    id: String,
    title: String,
    body: String,
    metadata: Value,
}

pub async fn run(
    state: Arc<AppState>,
    source: InboundSource,
    cancel: CancellationToken,
) -> AppResult<()> {
    let config: PollerConfig = source_config(&source)?;
    let base =
        crate::io_bus::ssrf::validate_target(&config.url, config.allow_private_network).await?;
    let interval = config.interval_secs.clamp(60, 86_400);
    loop {
        if cancel.is_cancelled() {
            break;
        }
        match poll_once(&state, &source, &config, &base).await {
            Ok(count) => {
                let _ = io_bus::mark_inbound(
                    &state.db,
                    &source.id,
                    if count > 0 { "TRIGGERED" } else { "NO_CHANGE" },
                    None,
                )
                .await;
            }
            Err(error) => {
                let message = format!("{error:#}");
                let _ =
                    io_bus::save_poller_state(&state.db, &source.id, None, None, Some(&message))
                        .await;
                let _ = io_bus::mark_inbound(&state.db, &source.id, "ERROR", Some(&message)).await;
                tracing::warn!(source = %source.id, error = %message, "http poller failed");
            }
        }
        tokio::select! {
            _ = cancel.cancelled() => break,
            _ = tokio::time::sleep(Duration::from_secs(interval)) => {}
        }
    }
    Ok(())
}

async fn poll_once(
    state: &Arc<AppState>,
    source: &InboundSource,
    config: &PollerConfig,
    base: &reqwest::Url,
) -> AppResult<usize> {
    let previous = io_bus::poller_state(&state.db, &source.id).await?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| AppError::Internal(format!("poller client: {error}")))?;
    let mut current = base.clone();
    let mut redirects = 0usize;
    let response = loop {
        let mut request = client.get(current.clone()).header(
            "accept",
            "application/rss+xml, application/atom+xml, application/json, text/html",
        );
        if let Some(etag) = previous.as_ref().and_then(|row| row.etag.as_deref()) {
            request = request.header("if-none-match", etag);
        }
        if let Some(modified) = previous
            .as_ref()
            .and_then(|row| row.last_modified.as_deref())
        {
            request = request.header("if-modified-since", modified);
        }
        if let Some(reference) = config
            .vault_ref
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let host = current.host_str().unwrap_or_default();
            let secret = crate::db::vault::resolve_for_edge(
                &state.db,
                reference,
                "sink",
                &source.id,
                None,
                Some(host),
                false,
            )
            .await?;
            request = request.bearer_auth(secret);
        }
        let response = request
            .send()
            .await
            .map_err(|error| AppError::Internal(format!("poller request failed: {error}")))?;
        if response.status().is_redirection() {
            redirects += 1;
            if redirects > 3 {
                return Err(AppError::BadRequest("poller excedeu 3 redirects".into()));
            }
            let location = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| AppError::BadRequest("redirect sem Location".into()))?;
            let next = current
                .join(location)
                .map_err(|error| AppError::BadRequest(format!("redirect inválido: {error}")))?;
            current =
                crate::io_bus::ssrf::validate_target(next.as_str(), config.allow_private_network)
                    .await?;
            continue;
        }
        break response;
    };
    let status = response.status();
    let etag = response
        .headers()
        .get("etag")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let modified = response
        .headers()
        .get("last-modified")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if status == reqwest::StatusCode::NOT_MODIFIED {
        io_bus::save_poller_state(
            &state.db,
            &source.id,
            etag.as_deref(),
            modified.as_deref(),
            None,
        )
        .await?;
        return Ok(0);
    }
    if !status.is_success() {
        return Err(AppError::Internal(format!(
            "poller respondeu {}",
            status.as_u16()
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| AppError::Internal(format!("poller body: {error}")))?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err(AppError::BadRequest(
            "resposta do poller excede 8 MiB".into(),
        ));
    }
    let body = String::from_utf8_lossy(&bytes);
    let items = parse_items(&config.format, &body, config, &current)?;
    let limit = config.max_items.unwrap_or(50).clamp(1, 200);
    let emit_on_change = config.emit_mode.as_deref() == Some("content_hash_change");
    let mut emitted = 0;
    for item in items.into_iter().take(limit) {
        let hash = content_hash(&format!("{}\n{}", item.title, item.body));
        let seen = io_bus::poller_seen_hash(&state.db, &source.id, &item.id).await?;
        if seen.as_deref() == Some(hash.as_str()) {
            continue;
        }
        if seen.is_some() && !emit_on_change {
            continue;
        }
        let event = NormalizedTaskEvent {
            source_id: source.id.clone(),
            external_id: item.id.clone(),
            title: item.title.clone(),
            body_markdown: item.body.clone(),
            priority: None,
            tags: vec!["http-poller".into(), config.format.clone()],
            attachments: Vec::new(),
            metadata: item.metadata,
        };
        crate::io_bus::ingest(state, source, event).await?;
        io_bus::mark_poller_seen(&state.db, &source.id, &item.id, &hash).await?;
        emitted += 1;
    }
    io_bus::prune_poller_seen(&state.db, &source.id, 500).await?;
    io_bus::save_poller_state(
        &state.db,
        &source.id,
        etag.as_deref(),
        modified.as_deref(),
        None,
    )
    .await?;
    Ok(emitted)
}

fn parse_items(
    format: &str,
    body: &str,
    config: &PollerConfig,
    base: &reqwest::Url,
) -> AppResult<Vec<PollerItem>> {
    match format.trim().to_lowercase().as_str() {
        "json" => parse_json(body, config),
        "html" => parse_html(body, config, base),
        _ => parse_feed(body),
    }
}

fn parse_feed(body: &str) -> AppResult<Vec<PollerItem>> {
    let feed = feed_rs::parser::parse(body.as_bytes())
        .map_err(|error| AppError::BadRequest(format!("feed inválido: {error}")))?;
    Ok(feed
        .entries
        .into_iter()
        .map(|entry| {
            let title = entry
                .title
                .map(|title| title.content)
                .unwrap_or_else(|| "Item do feed".into());
            let link = entry.links.first().map(|link| link.href.clone());
            let body = entry
                .summary
                .map(|summary| summary.content)
                .or_else(|| entry.content.and_then(|content| content.body))
                .unwrap_or_else(|| link.clone().unwrap_or_default());
            let id = link
                .clone()
                .unwrap_or_else(|| format!("{}-{}", entry.id, content_hash(&title)));
            PollerItem {
                id,
                title,
                body,
                metadata: serde_json::json!({ "link": link }),
            }
        })
        .collect())
}

fn parse_json(body: &str, config: &PollerConfig) -> AppResult<Vec<PollerItem>> {
    let value: Value = serde_json::from_str(body)
        .map_err(|error| AppError::BadRequest(format!("JSON do poller inválido: {error}")))?;
    let items = config
        .items_path
        .as_deref()
        .and_then(|path| json_path(&value, path))
        .or_else(|| value.as_array().map(|array| Value::Array(array.clone())))
        .or_else(|| value.get("items").cloned())
        .unwrap_or(Value::Null);
    let Some(items) = items.as_array() else {
        return Ok(Vec::new());
    };
    Ok(items
        .iter()
        .map(|item| {
            let title = config
                .title_path
                .as_deref()
                .and_then(|path| json_path(item, path))
                .and_then(|value| value.as_str().map(str::to_string))
                .or_else(|| {
                    item.get("title")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| "Item".into());
            let body = config
                .body_path
                .as_deref()
                .and_then(|path| json_path(item, path))
                .and_then(|value| value.as_str().map(str::to_string))
                .or_else(|| {
                    item.get("body")
                        .or_else(|| item.get("summary"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| item.to_string());
            let id = config
                .id_path
                .as_deref()
                .and_then(|path| json_path(item, path))
                .and_then(|value| value.as_str().map(str::to_string))
                .or_else(|| {
                    item.get("id")
                        .or_else(|| item.get("link"))
                        .or_else(|| item.get("url"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_else(|| content_hash(&format!("{title}{body}")));
            PollerItem {
                id,
                title,
                body,
                metadata: item.clone(),
            }
        })
        .collect())
}

fn parse_html(
    body: &str,
    config: &PollerConfig,
    base: &reqwest::Url,
) -> AppResult<Vec<PollerItem>> {
    let document = scraper::Html::parse_document(body);
    let selector =
        scraper::Selector::parse(config.selector.as_deref().unwrap_or("article, main, .item"))
            .map_err(|error| AppError::BadRequest(format!("selector HTML inválido: {error:?}")))?;
    let Ok(title_selector) = scraper::Selector::parse("h1, h2, h3, .title") else {
        return Err(AppError::Internal("selector de título inválido".into()));
    };
    let Ok(link_selector) = scraper::Selector::parse("a[href]") else {
        return Err(AppError::Internal("selector de link inválido".into()));
    };
    Ok(document
        .select(&selector)
        .map(|element| {
            let text = element
                .text()
                .collect::<Vec<_>>()
                .join(" ")
                .trim()
                .to_string();
            let title = element
                .select(&title_selector)
                .next()
                .map(|title| {
                    title
                        .text()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string()
                })
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| text.chars().take(120).collect());
            let href = element
                .select(&link_selector)
                .next()
                .and_then(|link| link.value().attr("href"))
                .and_then(|href| base.join(href).ok())
                .map(|url| url.to_string());
            let id = href
                .clone()
                .unwrap_or_else(|| content_hash(&format!("{title}{text}")));
            PollerItem {
                id,
                title,
                body: text,
                metadata: serde_json::json!({ "link": href }),
            }
        })
        .collect())
}

fn json_path(value: &Value, path: &str) -> Option<Value> {
    let path = serde_json_path::JsonPath::parse(path).ok()?;
    path.query(value)
        .all()
        .first()
        .map(|value| (*value).clone())
}

fn content_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
#[path = "http_poller_tests.rs"]
mod tests;
