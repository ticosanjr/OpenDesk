//! External MCP server pool (E2).
//!
//! Local servers are stdio child processes speaking line-delimited JSON-RPC.
//! Remote servers are plain JSON-RPC over HTTP. Tools are namespaced as
//! `server__tool` so two servers can expose the same name without colliding.
//!
//! The pool is intentionally small: one process per server, serialized calls,
//! a request timeout and one reconnect attempt. It is a bridge, not a full MCP
//! SDK implementation.

use crate::error::{AppError, AppResult};
use opendesk_engine::config_sync::{jsonc_editor, paths::StudioPaths};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::Path,
    process::Stdio,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::Mutex,
};

const CALL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub name: String,
    pub transport: String,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub disabled: bool,
}

impl ServerConfig {
    pub fn from_json(name: &str, value: &Value) -> Option<Self> {
        if value.get("disabled").and_then(Value::as_bool) == Some(true)
            || value.get("enabled").and_then(Value::as_bool) == Some(false)
        {
            return None;
        }
        let url = value.get("url").and_then(Value::as_str).map(str::to_string);
        let transport = value
            .get("type")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                if url.is_some() {
                    "remote".into()
                } else {
                    "local".into()
                }
            });
        let command = match value.get("command") {
            Some(Value::Array(items)) => items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            Some(Value::String(command)) => {
                command.split_whitespace().map(str::to_string).collect()
            }
            _ => Vec::new(),
        };
        if transport == "local" && command.is_empty() {
            return None;
        }
        if transport == "remote" && url.is_none() {
            return None;
        }
        Some(Self {
            name: name.to_string(),
            transport,
            command,
            url,
            headers: string_map(value.get("headers")),
            env: string_map(value.get("environment").or_else(|| value.get("env"))),
            disabled: false,
        })
    }

    fn key(&self, workspace_id: &str) -> String {
        format!("{workspace_id}:{}", self.name)
    }
}

fn string_map(value: Option<&Value>) -> HashMap<String, String> {
    value
        .and_then(Value::as_object)
        .map(|object| {
            object
                .iter()
                .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.into())))
                .collect()
        })
        .unwrap_or_default()
}

/// Reads global + workspace `opencode.jsonc`; workspace entries win by name.
pub fn load_configs(workspace_root: &Path) -> AppResult<Vec<ServerConfig>> {
    let mut by_name: HashMap<String, ServerConfig> = HashMap::new();
    for (paths, scope) in [
        (StudioPaths::new(None), "global"),
        (
            StudioPaths::new(Some(workspace_root.to_path_buf())),
            "workspace",
        ),
    ] {
        let document = jsonc_editor::read_for_scope(&paths, scope)?;
        for server in jsonc_editor::list_mcp_servers(&document) {
            if let Some(config) = ServerConfig::from_json(&server.name, &server.server) {
                by_name.insert(server.name, config);
            } else {
                by_name.remove(&server.name);
            }
        }
    }
    let mut configs: Vec<_> = by_name.into_values().collect();
    configs.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(configs)
}

struct ServerIo {
    _child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

struct ServerHandle {
    config: ServerConfig,
    io: Mutex<Option<ServerIo>>,
    next_id: AtomicU64,
    tools: Mutex<Vec<Value>>,
}

impl ServerHandle {
    fn new(config: ServerConfig) -> Self {
        Self {
            config,
            io: Mutex::new(None),
            next_id: AtomicU64::new(1),
            tools: Mutex::new(Vec::new()),
        }
    }

    async fn ensure(&self, root: &Path) -> AppResult<()> {
        if self.io.lock().await.is_some() {
            return Ok(());
        }
        match self.config.transport.as_str() {
            "remote" => {
                let tools = self.remote_request("tools/list", json!({})).await?;
                *self.tools.lock().await = extract_tools(&tools);
            }
            _ => {
                let mut child = Command::new(&self.config.command[0])
                    .args(&self.config.command[1..])
                    .current_dir(root)
                    .envs(&self.config.env)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .kill_on_drop(true)
                    .spawn()
                    .map_err(|error| {
                        AppError::Internal(format!("MCP {} não iniciou: {error}", self.config.name))
                    })?;
                let stdin = child
                    .stdin
                    .take()
                    .ok_or_else(|| AppError::Internal("MCP sem stdin".into()))?;
                let stdout = child
                    .stdout
                    .take()
                    .ok_or_else(|| AppError::Internal("MCP sem stdout".into()))?;
                *self.io.lock().await = Some(ServerIo {
                    _child: child,
                    stdin,
                    stdout: BufReader::new(stdout),
                });
                let init = self
                    .local_request(
                        "initialize",
                        json!({
                            "protocolVersion": "2024-11-05",
                            "capabilities": {},
                            "clientInfo": { "name": "opendesk", "version": env!("CARGO_PKG_VERSION") }
                        }),
                    )
                    .await?;
                let _ = init;
                let _ = self
                    .local_notify("notifications/initialized", json!({}))
                    .await;
                let tools = self.local_request("tools/list", json!({})).await?;
                *self.tools.lock().await = extract_tools(&tools);
            }
        }
        Ok(())
    }

    async fn local_notify(&self, method: &str, params: Value) -> AppResult<()> {
        let mut guard = self.io.lock().await;
        let io = guard
            .as_mut()
            .ok_or_else(|| AppError::Internal("MCP não conectado".into()))?;
        let request = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        io.stdin
            .write_all(format!("{request}\n").as_bytes())
            .await
            .map_err(|error| AppError::Internal(format!("MCP stdin: {error}")))?;
        io.stdin
            .flush()
            .await
            .map_err(|error| AppError::Internal(format!("MCP flush: {error}")))?;
        Ok(())
    }

    async fn local_request(&self, method: &str, params: Value) -> AppResult<Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let mut guard = self.io.lock().await;
        let io = guard
            .as_mut()
            .ok_or_else(|| AppError::Internal("MCP não conectado".into()))?;
        let request = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });
        io.stdin
            .write_all(format!("{request}\n").as_bytes())
            .await
            .map_err(|error| AppError::Internal(format!("MCP stdin: {error}")))?;
        io.stdin
            .flush()
            .await
            .map_err(|error| AppError::Internal(format!("MCP flush: {error}")))?;
        let deadline = tokio::time::Instant::now() + CALL_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return Err(AppError::Internal(format!(
                    "MCP {} timeout em {method}",
                    self.config.name
                )));
            }
            let mut line = String::new();
            let read = tokio::time::timeout(remaining, io.stdout.read_line(&mut line))
                .await
                .map_err(|_| AppError::Internal(format!("MCP {} timeout", self.config.name)))??;
            if read == 0 {
                return Err(AppError::Internal(format!(
                    "MCP {} encerrou",
                    self.config.name
                )));
            }
            let Ok(value) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if value.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if let Some(error) = value.get("error") {
                return Err(AppError::Internal(format!(
                    "MCP {}: {}",
                    self.config.name, error
                )));
            }
            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
        }
    }

    async fn remote_request(&self, method: &str, params: Value) -> AppResult<Value> {
        let url = self
            .config
            .url
            .as_deref()
            .ok_or_else(|| AppError::Internal("MCP remoto sem URL".into()))?;
        let client = reqwest::Client::new();
        let mut request = client.post(url).json(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        }));
        for (name, value) in &self.config.headers {
            request = request.header(name, value);
        }
        let response = request
            .timeout(CALL_TIMEOUT)
            .send()
            .await
            .map_err(|error| {
                AppError::Internal(format!("MCP {} HTTP: {error}", self.config.name))
            })?;
        let value: Value = response.json().await.map_err(|error| {
            AppError::Internal(format!("MCP {} JSON: {error}", self.config.name))
        })?;
        if let Some(error) = value.get("error") {
            return Err(AppError::Internal(format!(
                "MCP {}: {}",
                self.config.name, error
            )));
        }
        Ok(value.get("result").cloned().unwrap_or(Value::Null))
    }

    async fn call(&self, tool: &str, arguments: Value, root: &Path) -> AppResult<Value> {
        self.ensure(root).await?;
        let params = json!({ "name": tool, "arguments": arguments });
        let result = match self.config.transport.as_str() {
            "remote" => self.remote_request("tools/call", params).await,
            _ => self.local_request("tools/call", params).await,
        };
        if result.is_err() {
            self.reset().await;
        }
        result
    }

    async fn reset(&self) {
        *self.io.lock().await = None;
        self.tools.lock().await.clear();
    }
}

fn extract_tools(result: &Value) -> Vec<Value> {
    if let Some(tools) = result.get("tools").and_then(Value::as_array) {
        return tools.clone();
    }
    result.as_array().cloned().unwrap_or_default()
}

fn namespaced(server: &str, tool: &str) -> String {
    format!("{server}__{tool}")
}

pub fn split_namespaced(name: &str) -> Option<(&str, &str)> {
    let (server, tool) = name.split_once("__")?;
    (!server.is_empty() && !tool.is_empty()).then_some((server, tool))
}

#[derive(Default)]
pub struct McpPool {
    servers: Mutex<HashMap<String, std::sync::Arc<ServerHandle>>>,
}

impl McpPool {
    pub fn new() -> Self {
        Self::default()
    }

    async fn handle(
        &self,
        config: ServerConfig,
        workspace_id: &str,
    ) -> std::sync::Arc<ServerHandle> {
        let key = config.key(workspace_id);
        let mut servers = self.servers.lock().await;
        servers
            .entry(key)
            .or_insert_with(|| std::sync::Arc::new(ServerHandle::new(config)))
            .clone()
    }

    /// Connects enabled servers and returns their namespaced tools.
    pub async fn tools(&self, workspace_id: &str, workspace_root: &Path) -> AppResult<Vec<Value>> {
        let configs = load_configs(workspace_root)?;
        let mut tools = Vec::new();
        for config in configs {
            let handle = self.handle(config, workspace_id).await;
            if let Err(error) = handle.ensure(workspace_root).await {
                tracing::warn!(
                    server = %handle.config.name,
                    error = %error,
                    "external MCP unavailable"
                );
                continue;
            }
            for tool in handle.tools.lock().await.iter() {
                let Some(name) = tool.get("name").and_then(Value::as_str) else {
                    continue;
                };
                let mut tool = tool.clone();
                tool["name"] = Value::String(namespaced(&handle.config.name, name));
                let description = tool
                    .get("description")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                tool["description"] =
                    Value::String(format!("[{}] {description}", handle.config.name));
                tools.push(tool);
            }
        }
        Ok(tools)
    }

    pub async fn call(
        &self,
        workspace_id: &str,
        workspace_root: &Path,
        namespaced_name: &str,
        arguments: Value,
    ) -> AppResult<Value> {
        let (server, tool) = split_namespaced(namespaced_name)
            .ok_or_else(|| AppError::BadRequest(format!("tool MCP inválida: {namespaced_name}")))?;
        let config = load_configs(workspace_root)?
            .into_iter()
            .find(|config| config.name == server)
            .ok_or_else(|| AppError::NotFound(format!("servidor MCP {server}")))?;
        let handle = self.handle(config, workspace_id).await;
        handle.call(tool, arguments, workspace_root).await
    }

    pub async fn reset(&self) {
        self.servers.lock().await.clear();
    }
}

#[cfg(test)]
include!("tests.rs");
