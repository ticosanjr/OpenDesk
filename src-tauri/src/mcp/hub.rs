//! MCP Hub: a unified, read-mostly view over native `mcp.servers`.
//!
//! The source of truth remains `opencode.jsonc` in each scope. The hub adds a
//! curated catalog, static/runtime health and safe install/toggle/delete through
//! the same JSONC editor used by the Studio.

use crate::{
    app_state::AppState,
    config_sync::{jsonc_editor, paths::StudioPaths},
    db::workspaces,
    error::{AppError, AppResult},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};

#[derive(Debug, Clone, Serialize)]
pub struct HubServer {
    pub name: String,
    pub scope: String,
    pub workspace_id: String,
    pub transport: String,
    pub disabled: bool,
    pub command: Option<String>,
    pub url: Option<String>,
    pub server: Value,
    pub health: String,
    pub detail: Option<String>,
    pub runtime: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub transport: String,
    pub requires_token: bool,
    pub template: Value,
}

pub fn catalog() -> Vec<CatalogEntry> {
    let entry =
        |id: &str, name: &str, description: &str, transport: &str, template: Value| CatalogEntry {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            transport: transport.into(),
            requires_token: false,
            template,
        };
    vec![
        entry(
            "filesystem",
            "Filesystem",
            "Leitura/escrita controlada de um diretório. Aponte para o workspace.",
            "local",
            json!({
                "type": "local",
                "command": ["npx", "-y", "@modelcontextprotocol/server-filesystem", "{{workspace_root}}"],
                "disabled": false
            }),
        ),
        entry(
            "fetch",
            "Fetch",
            "Busca HTTP simples para páginas e APIs públicas.",
            "local",
            json!({
                "type": "local",
                "command": ["npx", "-y", "@modelcontextprotocol/server-fetch"],
                "disabled": false
            }),
        ),
        entry(
            "sqlite",
            "SQLite",
            "Consulta e edição de um banco SQLite local.",
            "local",
            json!({
                "type": "local",
                "command": ["npx", "-y", "@modelcontextprotocol/server-sqlite", "{{workspace_root}}/data.db"],
                "disabled": false
            }),
        ),
        entry(
            "playwright",
            "Playwright",
            "Navegação automatizada via Playwright MCP.",
            "local",
            json!({
                "type": "local",
                "command": ["npx", "-y", "@playwright/mcp@latest"],
                "disabled": false
            }),
        ),
        entry(
            "memory",
            "Memory graph",
            "Grafo de memória simples mantido pelo servidor oficial.",
            "local",
            json!({
                "type": "local",
                "command": ["npx", "-y", "@modelcontextprotocol/server-memory"],
                "disabled": false
            }),
        ),
    ]
}

pub async fn inventory(state: &AppState, workspace_id: Option<&str>) -> AppResult<Vec<HubServer>> {
    let workspace_id = match workspace_id {
        Some(id) => id.to_string(),
        None => crate::db::settings::active_workspace_id(&state.db)
            .await?
            .ok_or_else(|| AppError::BadRequest("nenhum workspace ativo".into()))?,
    };
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    let mut servers = Vec::new();
    let runtime = runtime_snapshot(state, Some(&workspace.root_path)).await;

    let global = StudioPaths::new(None);
    let global_doc = jsonc_editor::read_for_scope(&global, "global")?;
    for server in jsonc_editor::list_mcp_servers(&global_doc) {
        servers.push(to_hub(server, "global", &workspace_id, runtime.as_deref()));
    }

    let local = StudioPaths::new(Some(std::path::PathBuf::from(&workspace.root_path)));
    let workspace_doc = jsonc_editor::read_for_scope(&local, "workspace")?;
    for server in jsonc_editor::list_mcp_servers(&workspace_doc) {
        servers.push(to_hub(
            server,
            "workspace",
            &workspace_id,
            runtime.as_deref(),
        ));
    }
    servers.sort_by(|left, right| {
        left.scope
            .cmp(&right.scope)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(servers)
}

fn to_hub(
    server: jsonc_editor::McpServer,
    scope: &str,
    workspace_id: &str,
    runtime: Option<&[Value]>,
) -> HubServer {
    let command = server.server.get("command").and_then(|value| match value {
        Value::Array(items) => items.first().and_then(Value::as_str).map(str::to_string),
        Value::String(value) => Some(value.clone()),
        _ => None,
    });
    let url = server
        .server
        .get("url")
        .and_then(Value::as_str)
        .map(str::to_string);
    let disabled = server
        .server
        .get("disabled")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            server
                .server
                .get("enabled")
                .and_then(Value::as_bool)
                .map(|enabled| !enabled)
                .unwrap_or(false)
        });
    let runtime_entry = runtime.and_then(|list| {
        list.iter()
            .find(|entry| {
                entry.get("name").and_then(Value::as_str) == Some(server.name.as_str())
                    || entry.get("id").and_then(Value::as_str) == Some(server.name.as_str())
            })
            .cloned()
    });
    let (health, detail) = if disabled {
        ("disabled".to_string(), None)
    } else if command.is_none() && url.is_none() {
        (
            "invalid".to_string(),
            Some("servidor sem command nem url".to_string()),
        )
    } else if let Some(runtime) = runtime_entry.as_ref() {
        let status = runtime
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        (status, None)
    } else {
        ("configured".to_string(), None)
    };
    HubServer {
        name: server.name,
        scope: scope.to_string(),
        workspace_id: workspace_id.to_string(),
        transport: if url.is_some() { "remote" } else { "local" }.to_string(),
        disabled,
        command,
        url,
        server: server.server,
        health,
        detail,
        runtime: runtime_entry,
    }
}

async fn runtime_snapshot(state: &AppState, directory: Option<&str>) -> Option<Vec<Value>> {
    let client = state.daemon.client().await?;
    let mut list = client.mcp_servers(None).await.unwrap_or_default();
    if let Some(directory) = directory {
        list.extend(
            client
                .mcp_servers(Some(directory))
                .await
                .unwrap_or_default(),
        );
    }
    Some(list)
}

pub async fn config_path(
    state: &AppState,
    scope: &str,
    workspace_id: Option<&str>,
) -> AppResult<std::path::PathBuf> {
    let paths = match scope {
        "global" => StudioPaths::new(None),
        "workspace" => {
            let workspace_id = match workspace_id {
                Some(id) => id.to_string(),
                None => crate::db::settings::active_workspace_id(&state.db)
                    .await?
                    .ok_or_else(|| AppError::BadRequest("nenhum workspace ativo".into()))?,
            };
            let workspace = workspaces::get(&state.db, &workspace_id).await?;
            StudioPaths::new(Some(std::path::PathBuf::from(workspace.root_path)))
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "scope inválido '{other}'; use global ou workspace"
            )));
        }
    };
    paths.config_path(scope).map_err(Into::into)
}

#[derive(Debug, Deserialize)]
pub struct InstallInput {
    pub catalog_id: String,
    #[serde(default = "default_scope")]
    pub scope: String,
    #[serde(default)]
    pub workspace_id: Option<String>,
    /// Optional override for the catalog name.
    #[serde(default)]
    pub name: Option<String>,
}

fn default_scope() -> String {
    "workspace".into()
}

pub async fn install(state: &AppState, input: InstallInput) -> AppResult<HubServer> {
    let entry = catalog()
        .into_iter()
        .find(|entry| entry.id == input.catalog_id)
        .ok_or_else(|| AppError::NotFound(format!("catálogo {}", input.catalog_id)))?;
    let workspace_id = match input.workspace_id.clone() {
        Some(id) => id,
        None => crate::db::settings::active_workspace_id(&state.db)
            .await?
            .ok_or_else(|| AppError::BadRequest("nenhum workspace ativo".into()))?,
    };
    let workspace = workspaces::get(&state.db, &workspace_id).await?;
    let mut template = entry.template.clone();
    replace_placeholder(&mut template, "{{workspace_root}}", &workspace.root_path);
    let name = input.name.unwrap_or_else(|| entry.id.clone());
    let path = config_path(state, &input.scope, Some(&workspace_id)).await?;
    jsonc_editor::upsert_mcp_server(&path, &name, &template, None, &state.paths.backups_dir)?;
    let servers = inventory(state, Some(&workspace_id)).await?;
    servers
        .into_iter()
        .find(|server| server.name == name && server.scope == input.scope)
        .ok_or_else(|| AppError::Internal("servidor não apareceu no inventário".into()))
}

fn replace_placeholder(value: &mut Value, needle: &str, replacement: &str) {
    match value {
        Value::String(text) => *text = text.replace(needle, replacement),
        Value::Array(items) => {
            for item in items {
                replace_placeholder(item, needle, replacement);
            }
        }
        Value::Object(map) => {
            for item in map.values_mut() {
                replace_placeholder(item, needle, replacement);
            }
        }
        _ => {}
    }
}

pub async fn toggle(
    state: &AppState,
    scope: &str,
    name: &str,
    disabled: bool,
    workspace_id: Option<&str>,
) -> AppResult<()> {
    let path = config_path(state, scope, workspace_id).await?;
    let document = jsonc_editor::read_document(&path)?;
    let server = jsonc_editor::list_mcp_servers(&document)
        .into_iter()
        .find(|server| server.name == name)
        .ok_or_else(|| AppError::NotFound(format!("MCP {name} em {scope}")))?;
    let mut value = server.server;
    value["disabled"] = json!(disabled);
    value.as_object_mut().map(|map| map.remove("enabled"));
    jsonc_editor::upsert_mcp_server(&path, name, &value, None, &state.paths.backups_dir)?;
    Ok(())
}

pub async fn delete(state: &AppState, scope: &str, name: &str) -> AppResult<()> {
    let path = config_path(state, scope, None).await?;
    jsonc_editor::delete_mcp_server(&path, name, None, &state.paths.backups_dir)?;
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct TestResult {
    pub status: String,
    pub detail: Option<String>,
    pub command_found: Option<bool>,
    pub http_status: Option<u16>,
}

pub async fn test(
    state: &AppState,
    scope: &str,
    name: &str,
    workspace_id: Option<&str>,
) -> AppResult<TestResult> {
    let path = config_path(state, scope, workspace_id).await?;
    let document = jsonc_editor::read_document(&path)?;
    let server = jsonc_editor::list_mcp_servers(&document)
        .into_iter()
        .find(|server| server.name == name)
        .ok_or_else(|| AppError::NotFound(format!("MCP {name} em {scope}")))?;
    if server
        .server
        .get("disabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(TestResult {
            status: "disabled".into(),
            detail: Some("servidor desativado".into()),
            command_found: None,
            http_status: None,
        });
    }
    if let Some(url) = server.server.get("url").and_then(Value::as_str) {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .map_err(|error| AppError::Internal(error.to_string()))?;
        return match http.get(url).send().await {
            Ok(response) => Ok(TestResult {
                status: "reachable".into(),
                detail: Some(format!("HTTP {}", response.status())),
                command_found: None,
                http_status: Some(response.status().as_u16()),
            }),
            Err(error) => Ok(TestResult {
                status: "unreachable".into(),
                detail: Some(error.to_string()),
                command_found: None,
                http_status: None,
            }),
        };
    }
    let command = server.server.get("command").and_then(|value| match value {
        Value::Array(items) => items.first().and_then(Value::as_str),
        Value::String(value) => Some(value.as_str()),
        _ => None,
    });
    let Some(command) = command else {
        return Ok(TestResult {
            status: "invalid".into(),
            detail: Some("sem command".into()),
            command_found: None,
            http_status: None,
        });
    };
    let found = find_command(command);
    Ok(TestResult {
        status: if found { "ready" } else { "missing" }.into(),
        detail: if found {
            Some(format!("{command} encontrado no PATH"))
        } else {
            Some(format!(
                "{command} não encontrado; instale-o ou ajuste o comando"
            ))
        },
        command_found: Some(found),
        http_status: None,
    })
}

fn find_command(command: &str) -> bool {
    let path = Path::new(command);
    if path.is_absolute() || command.contains('/') {
        return path.is_file();
    }
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let candidate = dir.join(command);
                candidate.is_file() || cfg!(windows) && dir.join(format!("{command}.exe")).is_file()
            })
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_no_duplicate_ids_and_uses_placeholders() {
        let entries = catalog();
        let ids = entries
            .iter()
            .map(|entry| entry.id.clone())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(ids.len(), entries.len());
        assert!(
            entries
                .iter()
                .any(|entry| entry.template.to_string().contains("{{workspace_root}}"))
        );
    }

    #[test]
    fn finds_well_known_commands() {
        assert!(find_command("sh") || find_command("cmd"));
        assert!(!find_command("definitely-not-a-real-command-opendesk"));
    }
}
