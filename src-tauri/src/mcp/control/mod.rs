//! Operational MCP server exposed to OpenCode as `mcp.servers.opendesk-control`.
//!
//! It is intentionally a thin client over the local OpenDesk API: validation,
//! events, queueing and audit stay in the backend. Configuration changes for
//! agents/skills/commands/MCP files stay in `opendesk-config` proposals; this
//! server covers day-to-day operation (tasks, runs, routines, recipes, queue,
//! artifacts and the attention inbox).
//!
//! Safety boundaries:
//! - HITL checkpoints, permission requests and config proposals are read-only.
//! - New routines and recipes are always created disabled by the backend.
//! - Destructive deletion is not exposed in this first version.
//!
//! Tool domains live in sibling modules; this file only wires the MCP protocol
//! surface (`run_stdio`, `handle_http_with`) and fans out to them.

use crate::internal_api::{call, client, endpoint};
use crate::mcp_server;
use anyhow::{Context, Result, anyhow};
use reqwest::Method;
use serde_json::{Value, json};

mod artifacts;
mod attention;
mod chat;
mod dynamic;
mod handoffs;
mod interactions;
mod learning;
mod memory;
mod payload;
mod queue;
mod recipes;
mod schedules;
mod server;
mod tasks;
mod tools;
mod util;
mod voice;
mod worktrees;

pub use interactions::elicit_fallback;
use std::collections::HashMap;
use util::{optional, ws_path};

#[derive(Debug, Clone, Copy)]
enum DynamicKind {
    Recipe,
    Agent,
}

#[derive(Debug, Clone)]
struct DynamicTarget {
    kind: DynamicKind,
    id: String,
    tool: Value,
}

pub async fn run_stdio() -> Result<()> {
    let workspace_id = std::env::var("OPENDESK_WORKSPACE_ID")
        .context("OPENDESK_WORKSPACE_ID é obrigatório para o MCP de controle")?;
    let default_agent = std::env::var("OPENDESK_AGENT").unwrap_or_else(|_| "build".to_string());

    let phase = std::env::var("OPENDESK_MCP_PHASE").unwrap_or_else(|_| "ALL".into());
    let role = std::env::var("OPENDESK_MCP_ROLE").ok();
    let mut available = tools::filtered_tools(&phase, role.as_deref());
    let (dynamic_tools, dynamic_targets) = load_dynamic_tools(&workspace_id, &phase).await;
    available.extend(dynamic_tools);
    mcp_server::serve_stdio("opendesk-control", available, |name, arguments| {
        let workspace_id = &workspace_id;
        let default_agent = &default_agent;
        let dynamic_targets = dynamic_targets.clone();
        async move {
            dispatch(
                workspace_id,
                default_agent,
                &name,
                arguments,
                &dynamic_targets,
            )
            .await
            .map(|value| value.to_string())
        }
    })
    .await
}

/// Loads enabled recipes/agents so each one becomes a first-class MCP tool.
/// This is best-effort: if the local API is unavailable the static tools still
/// work.
async fn load_dynamic_tools(
    workspace_id: &str,
    phase: &str,
) -> (Vec<Value>, HashMap<String, DynamicTarget>) {
    let mut tools = Vec::new();
    let mut targets = HashMap::new();
    if phase.trim().eq_ignore_ascii_case("AUDIT") {
        return (tools, targets);
    }
    let endpoint = match endpoint() {
        Ok(endpoint) => endpoint,
        Err(error) => {
            eprintln!("OpenDesk control MCP: dynamic tools unavailable: {error:#}");
            return (tools, targets);
        }
    };
    let client = client();
    let recipes = call(
        &client,
        &endpoint,
        Method::GET,
        &ws_path("/api/recipes", workspace_id),
        None,
    )
    .await
    .ok()
    .and_then(|value| value.get("recipes").cloned())
    .and_then(|value| value.as_array().cloned())
    .unwrap_or_default();
    let agents = call(
        &client,
        &endpoint,
        Method::GET,
        &ws_path("/api/studio/agents", workspace_id),
        None,
    )
    .await
    .ok()
    .and_then(|value| value.as_array().cloned())
    .unwrap_or_default();

    tools.extend(tools::dynamic_tools(&recipes, &agents));
    let mut raw = HashMap::new();
    for recipe in &recipes {
        if let Some(id) = recipe.get("id").and_then(Value::as_str)
            && recipe
                .get("is_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(true)
        {
            raw.insert(
                tools::tool_name("recipe", id),
                (DynamicKind::Recipe, id.to_string()),
            );
        }
    }
    for agent in &agents {
        if let Some(id) = agent.get("id").and_then(Value::as_str)
            && !agent
                .get("disabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        {
            raw.insert(
                tools::tool_name("agent", id),
                (DynamicKind::Agent, id.to_string()),
            );
        }
    }
    for tool in &tools {
        let Some(name) = tool.get("name").and_then(Value::as_str) else {
            continue;
        };
        if let Some((kind, id)) = raw.remove(name) {
            targets.insert(
                name.to_string(),
                DynamicTarget {
                    kind,
                    id,
                    tool: tool.clone(),
                },
            );
        }
    }
    (tools, targets)
}

/// HTTP gateway entry point used by `/api/mcp`. It shares the same tool list,
/// dynamic recipe/agent tools and dispatch path as the stdio MCP server.
#[allow(clippy::too_many_arguments)]
pub async fn handle_http_with(
    workspace_id: &str,
    default_agent: &str,
    phase: &str,
    role: &str,
    policy: Option<&crate::mcp_rbac::ToolPolicy>,
    pool: Option<&crate::mcp_pool::McpPool>,
    workspace_root: Option<&std::path::Path>,
    request: Value,
) -> Value {
    // A role narrows an "ALL" session to the phase it represents. Explicit
    // phase headers still win, so callers can opt into a specific view.
    let phase = if phase.trim().eq_ignore_ascii_case("ALL") && !role.trim().is_empty() {
        match role.trim().to_lowercase().as_str() {
            "read" => "AUDIT",
            "audit" => "AUDIT",
            "plan" => "PLAN",
            "build" => "BUILD",
            "ops" | "operator" => "OPS",
            "chat" => "CHAT",
            _ => phase,
        }
    } else {
        phase
    };
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let params = request.get("params").cloned().unwrap_or(Value::Null);
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "opendesk-control", "version": env!("CARGO_PKG_VERSION") }
        })),
        "notifications/initialized" => Ok(Value::Null),
        "tools/list" => {
            let mut available = tools::filtered_tools(phase, None);
            let (dynamic, _) = load_dynamic_tools(workspace_id, phase).await;
            available.extend(dynamic);
            if let (Some(pool), Some(root)) = (pool, workspace_root) {
                match pool.tools(workspace_id, root).await {
                    Ok(external) => available.extend(external),
                    Err(error) => {
                        tracing::warn!(error = %error, "external MCP tools unavailable")
                    }
                }
            }
            if let Some(policy) = policy {
                available.retain(|tool| {
                    tool.get("name")
                        .and_then(Value::as_str)
                        .is_some_and(|name| policy.allows(name))
                });
            }
            Ok(json!({ "tools": available }))
        }
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .map(str::to_string);
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match name {
                Some(name) if policy.is_some_and(|policy| !policy.allows(&name)) => {
                    Err(anyhow!("agente sem permissão para a tool {name}"))
                }
                Some(name) => {
                    if let (Some(pool), Some(root)) = (pool, workspace_root)
                        && crate::mcp_pool::split_namespaced(&name).is_some()
                    {
                        return match pool.call(workspace_id, root, &name, arguments).await {
                            Ok(value) => json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "content": [{ "type": "text", "text": value.to_string() }],
                                    "isError": false,
                                }
                            }),
                            Err(error) => json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": { "code": -32603, "message": format!("{error:#}") }
                            }),
                        };
                    }
                    // Static tools do not need the two internal API calls used to
                    // discover recipe/agent tools. Only dynamic names pay that cost.
                    let dynamic = name.starts_with("recipe_") || name.starts_with("agent_");
                    let (_, targets) = if dynamic {
                        load_dynamic_tools(workspace_id, phase).await
                    } else {
                        (Vec::new(), std::collections::HashMap::new())
                    };
                    dispatch(workspace_id, default_agent, &name, arguments, &targets)
                        .await
                        .map(|value| {
                            json!({
                                "content": [{ "type": "text", "text": value.to_string() }],
                                "isError": false,
                            })
                        })
                }
                None => Err(anyhow!("params.name é obrigatório")),
            }
        }
        other => Err(anyhow!("método MCP não suportado: {other}")),
    };
    match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(error) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": format!("{error:#}") },
        }),
    }
}
async fn dispatch(
    workspace_id: &str,
    default_agent: &str,
    tool: &str,
    args: Value,
    dynamic: &HashMap<String, DynamicTarget>,
) -> Result<Value> {
    if let Some(target) = dynamic.get(tool) {
        let endpoint = endpoint()?;
        let client = client();
        return match target.kind {
            DynamicKind::Recipe => {
                dynamic::run_dynamic_recipe(&client, &endpoint, workspace_id, &target.id, &args)
                    .await
            }
            DynamicKind::Agent => {
                dynamic::run_dynamic_agent(&client, &endpoint, workspace_id, &target.id, &args)
                    .await
            }
        };
    }
    // Tool discovery is local; do not require a running local API for it.
    if tool == "mcp_discover_tools" {
        let query = optional(&args, "query").unwrap_or_default();
        let phase = optional(&args, "phase");
        let mut found = tools::discover(&query, phase.as_deref());
        let query = query.trim().to_lowercase();
        for target in dynamic.values() {
            let name = target
                .tool
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("");
            let description = target
                .tool
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or("");
            let matches = query.is_empty()
                || name.to_lowercase().contains(&query)
                || description.to_lowercase().contains(&query);
            if matches {
                found.push(target.tool.clone());
            }
        }
        return Ok(json!({ "tools": found }));
    }
    let endpoint = endpoint()?;
    let client = client();
    let args = &args;
    if let Some(result) =
        server::dispatch(&client, &endpoint, workspace_id, default_agent, tool, args).await
    {
        return result;
    }
    if let Some(result) =
        tasks::dispatch(&client, &endpoint, workspace_id, default_agent, tool, args).await
    {
        return result;
    }
    if let Some(result) =
        interactions::dispatch(&client, &endpoint, workspace_id, default_agent, tool, args).await
    {
        return result;
    }
    if let Some(result) =
        memory::dispatch(&client, &endpoint, workspace_id, default_agent, tool, args).await
    {
        return result;
    }
    if let Some(result) =
        schedules::dispatch(&client, &endpoint, workspace_id, default_agent, tool, args).await
    {
        return result;
    }
    if let Some(result) =
        recipes::dispatch(&client, &endpoint, workspace_id, default_agent, tool, args).await
    {
        return result;
    }
    if let Some(result) =
        attention::dispatch(&client, &endpoint, workspace_id, default_agent, tool, args).await
    {
        return result;
    }
    if let Some(result) =
        queue::dispatch(&client, &endpoint, workspace_id, default_agent, tool, args).await
    {
        return result;
    }
    if let Some(result) =
        artifacts::dispatch(&client, &endpoint, workspace_id, default_agent, tool, args).await
    {
        return result;
    }
    if let Some(result) =
        handoffs::dispatch(&client, &endpoint, workspace_id, default_agent, tool, args).await
    {
        return result;
    }
    if let Some(result) =
        learning::dispatch(&client, &endpoint, workspace_id, default_agent, tool, args).await
    {
        return result;
    }
    if let Some(result) =
        chat::dispatch(&client, &endpoint, workspace_id, default_agent, tool, args).await
    {
        return result;
    }
    if let Some(result) =
        voice::dispatch(&client, &endpoint, workspace_id, default_agent, tool, args).await
    {
        return result;
    }
    if let Some(result) =
        worktrees::dispatch(&client, &endpoint, workspace_id, default_agent, tool, args).await
    {
        return result;
    }
    Err(anyhow!("ferramenta desconhecida: {tool}"))
}
