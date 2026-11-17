//! External MCP server pool (E2).
//!
//! Local servers are stdio child processes speaking line-delimited JSON-RPC.
//! Remote servers are plain JSON-RPC over HTTP. Tools are namespaced as
//! `server__tool` so two servers can expose the same name without colliding.
//!
//! The pool is intentionally small: one process per server, serialized calls,
//! a request timeout and one reconnect attempt. It is a bridge, not a full MCP
//! SDK implementation.

use crate::{
    config_sync::{jsonc_editor, paths::StudioPaths},
    error::{AppError, AppResult},
};
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
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fake_server() -> Option<(tempfile::TempDir, PathBuf)> {
        let dir = tempfile::TempDir::new().ok()?;
        let script = dir.path().join("fake_mcp.py");
        std::fs::write(
            &script,
            r#"import json, sys
for line in sys.stdin:
    req = json.loads(line)
    method = req.get("method")
    if method == "initialize":
        result = {"protocolVersion": "2024-11-05", "capabilities": {"tools": {}}, "serverInfo": {"name": "fake", "version": "1"}}
    elif method == "tools/list":
        result = {"tools": [{"name": "echo", "description": "echo", "inputSchema": {"type": "object"}}]}
    elif method == "tools/call":
        args = req.get("params", {}).get("arguments", {})
        result = {"content": [{"type": "text", "text": json.dumps(args)}], "isError": False}
    else:
        result = {}
    print(json.dumps({"jsonrpc": "2.0", "id": req.get("id"), "result": result}), flush=True)
"#,
        )
        .ok()?;
        Some((dir, script))
    }

    #[test]
    fn namespaces_and_splits_tools() {
        assert_eq!(namespaced("fs", "read"), "fs__read");
        assert_eq!(split_namespaced("fs__read"), Some(("fs", "read")));
        assert_eq!(split_namespaced("plain"), None);
        assert_eq!(split_namespaced("__bad"), None);
    }

    #[tokio::test]
    async fn connects_lists_and_calls_a_stdio_server() {
        let Some((dir, script)) = fake_server() else {
            return;
        };
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        let config = ServerConfig {
            name: "fake".into(),
            transport: "local".into(),
            command: vec!["python3".into(), script.to_string_lossy().to_string()],
            url: None,
            headers: HashMap::new(),
            env: HashMap::new(),
            disabled: false,
        };
        let handle = ServerHandle::new(config);
        handle.ensure(dir.path()).await.expect("connect");
        let tools = handle.tools.lock().await.clone();
        assert_eq!(tools[0]["name"], "echo");
        let result = handle
            .call("echo", json!({ "text": "oi" }), dir.path())
            .await
            .expect("call");
        assert!(
            result["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("oi")
        );
    }

    #[tokio::test]
    async fn aggregates_two_servers_with_distinct_namespaces() {
        let Some((dir, script)) = fake_server() else {
            return;
        };
        if std::process::Command::new("python3")
            .arg("--version")
            .output()
            .is_err()
        {
            return;
        }
        std::fs::write(
            dir.path().join("opencode.jsonc"),
            format!(
                r#"{{"mcp":{{"servers":{{"one":{{"type":"local","command":["python3","{}"]}},"two":{{"type":"local","command":["python3","{}"]}}}}}}}}"#,
                script.display(),
                script.display()
            ),
        )
        .expect("config");
        let pool = McpPool::new();
        let tools = pool.tools("ws", dir.path()).await.expect("tools");
        let names: Vec<_> = tools
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect();
        assert!(names.contains(&"one__echo"), "{names:?}");
        assert!(names.contains(&"two__echo"), "{names:?}");
        let one = pool
            .call("ws", dir.path(), "one__echo", json!({ "side": "one" }))
            .await
            .expect("one");
        let two = pool
            .call("ws", dir.path(), "two__echo", json!({ "side": "two" }))
            .await
            .expect("two");
        assert!(one["content"][0]["text"].as_str().unwrap().contains("one"));
        assert!(two["content"][0]["text"].as_str().unwrap().contains("two"));
    }

    #[tokio::test]
    async fn loads_workspace_config_and_skips_disabled() {
        let dir = tempfile::TempDir::new().expect("temp");
        std::fs::write(
            dir.path().join("opencode.jsonc"),
            r#"{"mcp":{"servers":{"fake":{"type":"local","command":["python3","x.py"]},"off":{"type":"local","command":["python3","y.py"],"disabled":true}}}}"#,
        )
        .expect("config");
        let configs = load_configs(dir.path()).expect("configs");
        assert!(configs.iter().any(|config| config.name == "fake"));
        assert!(!configs.iter().any(|config| config.name == "off"));
    }
}
