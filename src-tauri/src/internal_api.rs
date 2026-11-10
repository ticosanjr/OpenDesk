//! Thin client for the local OpenDesk HTTP API.
//!
//! Both the headless CLI and the MCP control bridge talk to the running app so
//! validation, events, queueing and audit stay in one place. Callers must set
//! `OPENDESK_DATA_DIR`/`OPENDESK_CONFIG_DIR` (the MCP bridge receives them from
//! the workspace configuration).

use crate::{app_state::ApiEndpoint, paths::AppPaths};
use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Method};
use serde_json::Value;

pub fn endpoint() -> Result<ApiEndpoint> {
    let paths =
        AppPaths::discover().context("não foi possível localizar os diretórios do OpenDesk")?;
    let raw = std::fs::read_to_string(&paths.api_lock_path).with_context(|| {
        format!(
            "OpenDesk não está em execução ({} ausente). Abra o app ou rode com --headless.",
            paths.api_lock_path.display()
        )
    })?;
    serde_json::from_str(&raw).context("api.json inválido")
}

pub fn client() -> Client {
    Client::new()
}

pub async fn call(
    client: &Client,
    endpoint: &ApiEndpoint,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value> {
    let mut request = client
        .request(method, format!("http://127.0.0.1:{}{path}", endpoint.port))
        .bearer_auth(&endpoint.token);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request
        .send()
        .await
        .context("falha ao chamar a API local")?;
    let status = response.status();
    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        let message = serde_json::from_str::<Value>(&text)
            .ok()
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or(text);
        return Err(anyhow!("API {}: {}", status.as_u16(), message));
    }
    Ok(response.json().await.unwrap_or(Value::Null))
}
