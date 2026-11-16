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

use crate::app_state::ApiEndpoint;
use crate::internal_api::{call, client, endpoint};
use crate::mcp_server;
use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Method};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

mod payload;
mod tools;
mod util;

use payload::{routine_body, routine_update_body, task_create_body, task_update_body};
use std::collections::HashMap;
use util::{bool_arg, integer_arg, optional, optional_str, required, ws_path};

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
pub async fn handle_http(
    workspace_id: &str,
    default_agent: &str,
    phase: &str,
    request: Value,
) -> Value {
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
                Some(name) => {
                    let (_, targets) = load_dynamic_tools(workspace_id, phase).await;
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
                run_dynamic_recipe(&client, &endpoint, workspace_id, &target.id, &args).await
            }
            DynamicKind::Agent => {
                run_dynamic_agent(&client, &endpoint, workspace_id, &target.id, &args).await
            }
        };
    }
    let endpoint = endpoint()?;
    let client = client();
    match tool {
        "opendesk_status" => status(&client, &endpoint, workspace_id).await,
        "task_list" => task_list(&client, &endpoint, workspace_id, &args).await,
        "task_create" => {
            let body = task_create_body(&args)?;
            call(
                &client,
                &endpoint,
                Method::POST,
                &ws_path("/api/tasks", workspace_id),
                Some(body),
            )
            .await
        }
        "task_update" => {
            let id = required(&args, "id")?;
            let body = task_update_body(&args)?;
            call(
                &client,
                &endpoint,
                Method::PATCH,
                &format!("/api/tasks/{id}"),
                Some(body),
            )
            .await
        }
        "task_move" => {
            let id = required(&args, "id")?;
            let body = json!({
                "status": required(&args, "status")?,
                "reason": optional(&args, "reason"),
            });
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/tasks/{id}/transition"),
                Some(body),
            )
            .await
        }
        "ask_user" => ask_user(&client, &endpoint, &args).await,
        "memory_save" => {
            let agent = optional(&args, "agent").unwrap_or_else(|| default_agent.to_string());
            let mut path = ws_path("/api/memory/notes", workspace_id);
            path.push_str(&format!("&agent={}", urlencoding::encode(&agent)));
            let body = json!({
                "namespace": optional(&args, "namespace").unwrap_or_else(|| "core".into()),
                "kind": optional(&args, "kind").unwrap_or_else(|| "semantic".into()),
                "title": required(&args, "title")?,
                "body": required(&args, "body")?,
                "tags": util::string_array(&args, "tags"),
                "confidence": args.get("confidence").and_then(Value::as_f64).unwrap_or(0.6),
                "trust_level": optional(&args, "trust_level").unwrap_or_else(|| "observed".into()),
                "expires_at": optional(&args, "expires_at"),
                "source": optional(&args, "source"),
            });
            call(&client, &endpoint, Method::POST, &path, Some(body)).await
        }
        "memory_search" => {
            let agent = optional(&args, "agent").unwrap_or_else(|| default_agent.to_string());
            let query = required(&args, "query")?;
            let mut path = ws_path("/api/memory/search", workspace_id);
            path.push_str(&format!(
                "&q={}&agent={}",
                urlencoding::encode(query),
                urlencoding::encode(&agent)
            ));
            if let Some(namespace) = optional(&args, "namespace") {
                path.push_str(&format!("&namespace={}", urlencoding::encode(&namespace)));
            }
            if let Some(limit) = integer_arg(&args, "limit") {
                path.push_str(&format!("&limit={limit}"));
            }
            call(&client, &endpoint, Method::GET, &path, None).await
        }
        "mcp_discover_tools" => {
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
            Ok(json!({ "tools": found }))
        }
        "recipe_resume" => {
            let id = required(&args, "id")?;
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/recipes/runs/{id}/retry"),
                None,
            )
            .await
        }
        "recipe_cancel" => {
            let id = required(&args, "id")?;
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/recipes/runs/{id}/cancel"),
                None,
            )
            .await
        }
        "voice_speak" => {
            let body = json!({
                "text": required(&args, "text")?,
                "workspace_id": workspace_id,
            });
            call(
                &client,
                &endpoint,
                Method::POST,
                "/api/voice/speak",
                Some(body),
            )
            .await
        }
        "handoff_create" => {
            let body = json!({
                "source_agent": required(&args, "source_agent")?,
                "target_agent": required(&args, "target_agent")?,
                "objective": required(&args, "objective")?,
                "task_id": optional(&args, "task_id"),
                "parent_handoff_id": optional(&args, "parent_handoff_id"),
                "input_artifacts": util::string_array(&args, "input_artifacts"),
                "output_schema": args.get("output_schema").cloned().unwrap_or(Value::Null),
                "acceptance": util::string_array(&args, "acceptance"),
                "sub_budget_usd": args.get("sub_budget_usd").and_then(Value::as_f64),
            });
            call(
                &client,
                &endpoint,
                Method::POST,
                &ws_path("/api/handoffs", workspace_id),
                Some(body),
            )
            .await
        }
        "handoff_list" => {
            let mut path = ws_path("/api/handoffs", workspace_id);
            if let Some(status) = optional(&args, "status") {
                path.push_str(&format!("&status={}", urlencoding::encode(&status)));
            }
            if let Some(limit) = integer_arg(&args, "limit") {
                path.push_str(&format!("&limit={limit}"));
            }
            call(&client, &endpoint, Method::GET, &path, None).await
        }
        "handoff_respond" => {
            let id = required(&args, "id")?;
            match required(&args, "decision")?.to_lowercase().as_str() {
                "accept" => {
                    let body = json!({
                        "note": optional(&args, "note"),
                        "force": bool_arg(&args, "force").unwrap_or(false),
                    });
                    call(
                        &client,
                        &endpoint,
                        Method::POST,
                        &format!("/api/handoffs/{id}/accept"),
                        Some(body),
                    )
                    .await
                }
                "reject" => {
                    let body = json!({ "reason": optional(&args, "reason") });
                    call(
                        &client,
                        &endpoint,
                        Method::POST,
                        &format!("/api/handoffs/{id}/reject"),
                        Some(body),
                    )
                    .await
                }
                other => Err(anyhow!("decisão inválida: {other}; use accept ou reject")),
            }
        }
        "handoff_finish" => {
            let id = required(&args, "id")?;
            let body = json!({
                "status": required(&args, "status")?,
                "result": args.get("result").cloned().unwrap_or(Value::Null),
            });
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/handoffs/{id}/finish"),
                Some(body),
            )
            .await
        }
        "learning_save" => {
            let body = json!({
                "title": required(&args, "title")?,
                "detail": required(&args, "detail")?,
                "category": optional(&args, "category"),
                "severity": optional(&args, "severity"),
                "agent_name": optional(&args, "agent_name"),
                "remember": bool_arg(&args, "remember").unwrap_or(false),
                "workspace_id": optional(&args, "workspace_id"),
            });
            call(
                &client,
                &endpoint,
                Method::POST,
                "/api/learning/lessons",
                Some(body),
            )
            .await
        }
        "personal_context_save" => {
            let body = json!({
                "scope": optional(&args, "scope").unwrap_or_else(|| "workspace".into()),
                "name": optional(&args, "name"),
                "instructions": required(&args, "instructions")?,
                "user_request": required(&args, "user_request")?,
                "is_enabled": true,
                "source_quote": required(&args, "user_request")?,
                "workspace_id": optional(&args, "workspace_id"),
            });
            call(
                &client,
                &endpoint,
                Method::POST,
                "/api/personal-context",
                Some(body),
            )
            .await
        }
        "chat_send_file" => {
            let thread_id = required(&args, "thread_id")?;
            let body = json!({
                "path": required(&args, "path")?,
                "caption": optional(&args, "caption"),
            });
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/chat/threads/{thread_id}/files"),
                Some(body),
            )
            .await
        }
        "task_dispatch" => {
            let id = required(&args, "id")?;
            let body = json!({
                "policy": optional(&args, "policy"),
                "manual_agent": optional(&args, "manual_agent"),
                "manual_model": optional(&args, "manual_model"),
                "manual_effort": optional(&args, "manual_effort"),
                "execution_mode": optional(&args, "execution_mode"),
            });
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/tasks/{id}/dispatch"),
                Some(body),
            )
            .await
        }
        "task_message_list" => {
            let id = required(&args, "id")?;
            call(
                &client,
                &endpoint,
                Method::GET,
                &format!("/api/tasks/{id}/messages"),
                None,
            )
            .await
        }
        "task_message_post" => {
            let id = required(&args, "id")?;
            let body = json!({
                "content": required(&args, "content")?,
                "run_id": optional(&args, "run_id"),
                "continue_execution": bool_arg(&args, "continue_execution").unwrap_or(false),
                "agent": optional(&args, "agent"),
                "model": optional(&args, "model"),
            });
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/tasks/{id}/messages"),
                Some(body),
            )
            .await
        }
        "run_list" => run_list(&client, &endpoint, workspace_id, &args).await,
        "routine_list" => {
            if let Some(id) = optional_str(&args, "id") {
                call(
                    &client,
                    &endpoint,
                    Method::GET,
                    &format!("/api/schedules/{id}"),
                    None,
                )
                .await
            } else {
                call(
                    &client,
                    &endpoint,
                    Method::GET,
                    &ws_path("/api/schedules", workspace_id),
                    None,
                )
                .await
            }
        }
        "routine_create" => {
            let body = routine_body(workspace_id, &args)?;
            call(
                &client,
                &endpoint,
                Method::POST,
                &ws_path("/api/schedules", workspace_id),
                Some(body),
            )
            .await
        }
        "routine_update" => {
            let id = required(&args, "id")?;
            let body = routine_update_body(&args)?;
            call(
                &client,
                &endpoint,
                Method::PATCH,
                &format!("/api/schedules/{id}"),
                Some(body),
            )
            .await
        }
        "routine_set_active" => {
            let id = required(&args, "id")?;
            let active = bool_arg(&args, "active")
                .ok_or_else(|| anyhow!("argumento 'active' é obrigatório"))?;
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/schedules/{id}/active"),
                Some(json!({ "active": active })),
            )
            .await
        }
        "routine_run" => {
            let id = required(&args, "id")?;
            let dry_run = bool_arg(&args, "dry_run").unwrap_or(false);
            let path = if dry_run {
                format!("/api/schedules/{id}/dry-run")
            } else {
                format!("/api/schedules/{id}/run")
            };
            call(&client, &endpoint, Method::POST, &path, Some(json!({}))).await
        }
        "recipe_list" => recipe_list(&client, &endpoint, workspace_id, &args).await,
        "recipe_save" => {
            let id = required(&args, "id")?;
            let yaml = required(&args, "yaml")?;
            call(
                &client,
                &endpoint,
                Method::PUT,
                &ws_path(&format!("/api/recipes/{id}"), workspace_id),
                Some(json!({ "yaml": yaml })),
            )
            .await
        }
        "recipe_set_enabled" => {
            let id = required(&args, "id")?;
            let enabled = bool_arg(&args, "enabled")
                .ok_or_else(|| anyhow!("argumento 'enabled' é obrigatório"))?;
            call(
                &client,
                &endpoint,
                Method::POST,
                &ws_path(&format!("/api/recipes/{id}/enabled"), workspace_id),
                Some(json!({ "enabled": enabled })),
            )
            .await
        }
        "recipe_run" => {
            let id = required(&args, "id")?;
            let body = json!({
                "trigger_context": args
                    .get("trigger_context")
                    .cloned()
                    .filter(|value| !value.is_null())
                    .unwrap_or_else(|| json!({ "manual": true })),
                "dry_run": bool_arg(&args, "dry_run").unwrap_or(false),
            });
            call(
                &client,
                &endpoint,
                Method::POST,
                &ws_path(&format!("/api/recipes/{id}/run"), workspace_id),
                Some(body),
            )
            .await
        }
        "attention_list" => call(
            &client,
            &endpoint,
            Method::GET,
            &ws_path("/api/attention", workspace_id),
            None,
        )
        .await
        .map(attention_summary),
        "queue_control" => queue_control(&client, &endpoint, workspace_id, &args).await,
        "artifact_list" => artifact_list(&client, &endpoint, workspace_id, &args).await,
        "artifact_read" => {
            let id = required(&args, "id")?;
            let max_bytes = integer_arg(&args, "max_bytes")
                .unwrap_or(65_536)
                .clamp(1024, 2 * 1024 * 1024);
            call(
                &client,
                &endpoint,
                Method::GET,
                &format!("/api/artifacts/{id}/content?max_bytes={max_bytes}"),
                None,
            )
            .await
        }
        "artifact_pin" => {
            let id = required(&args, "id")?;
            let pinned = bool_arg(&args, "pinned")
                .ok_or_else(|| anyhow!("argumento 'pinned' é obrigatório"))?;
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/artifacts/{id}/pin"),
                Some(json!({ "pinned": pinned })),
            )
            .await
        }
        "artifact_rollback" => {
            let id = required(&args, "id")?;
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/artifacts/{id}/rollback"),
                Some(json!({})),
            )
            .await
        }
        "recipe_versions" => {
            let id = required(&args, "id")?;
            call(
                &client,
                &endpoint,
                Method::GET,
                &ws_path(&format!("/api/recipes/{id}/versions"), workspace_id),
                None,
            )
            .await
        }
        "recipe_diff" => {
            let id = required(&args, "id")?;
            let version = required(&args, "version_id")?;
            let mut path = ws_path(
                &format!("/api/recipes/{id}/versions/{version}/diff"),
                workspace_id,
            );
            if let Some(against) = optional(&args, "against") {
                path.push_str(&format!("&against={}", urlencoding::encode(&against)));
            }
            call(&client, &endpoint, Method::GET, &path, None).await
        }
        "recipe_rollback" => {
            let id = required(&args, "id")?;
            let version_id = required(&args, "version_id")?;
            call(
                &client,
                &endpoint,
                Method::POST,
                &ws_path(&format!("/api/recipes/{id}/rollback"), workspace_id),
                Some(json!({ "version_id": version_id })),
            )
            .await
        }
        "worktree_list" => {
            call(
                &client,
                &endpoint,
                Method::GET,
                &ws_path("/api/worktrees", workspace_id),
                None,
            )
            .await
        }
        "worktree_prepare" => {
            call(
                &client,
                &endpoint,
                Method::POST,
                &ws_path("/api/worktrees", workspace_id),
                Some(json!({ "run_id": optional(&args, "run_id") })),
            )
            .await
        }
        "worktree_diff" => {
            let id = required(&args, "id")?;
            call(
                &client,
                &endpoint,
                Method::GET,
                &format!("/api/worktrees/{id}/diff"),
                None,
            )
            .await
        }
        "worktree_merge" => {
            let id = required(&args, "id")?;
            let body = json!({ "approved": bool_arg(&args, "approved").unwrap_or(false) });
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/worktrees/{id}/merge"),
                Some(body),
            )
            .await
        }
        "worktree_cleanup" => {
            let id = required(&args, "id")?;
            call(
                &client,
                &endpoint,
                Method::DELETE,
                &format!("/api/worktrees/{id}"),
                None,
            )
            .await
        }
        other => Err(anyhow!("ferramenta desconhecida: {other}")),
    }
}

async fn status(client: &Client, endpoint: &ApiEndpoint, workspace_id: &str) -> Result<Value> {
    let health = call(client, endpoint, Method::GET, "/api/health", None).await?;
    let queue = call(
        client,
        endpoint,
        Method::GET,
        &ws_path("/api/queue/status", workspace_id),
        None,
    )
    .await?;
    let dashboard = call(
        client,
        endpoint,
        Method::GET,
        &ws_path("/api/dashboard", workspace_id),
        None,
    )
    .await?;
    let attention = call(
        client,
        endpoint,
        Method::GET,
        &ws_path("/api/attention", workspace_id),
        None,
    )
    .await?;
    Ok(json!({
        "workspace_id": workspace_id,
        "health": health,
        "queue": queue,
        "dashboard": dashboard,
        "attention": attention_summary(attention),
    }))
}

fn attention_summary(value: Value) -> Value {
    let count = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_array)
            .map(|items| items.len())
            .unwrap_or(0)
    };
    json!({
        "awaiting_approval": count("awaiting_approval"),
        "blocked": count("blocked"),
        "failed_runs": count("failed_runs"),
        "requests": count("requests"),
        "hitl": count("hitl"),
        "proposals": count("proposals"),
        "items": value,
    })
}

async fn task_list(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    if let Some(id) = optional_str(args, "id") {
        return call(
            client,
            endpoint,
            Method::GET,
            &format!("/api/tasks/{id}"),
            None,
        )
        .await;
    }
    let tasks = call(
        client,
        endpoint,
        Method::GET,
        &ws_path("/api/tasks", workspace_id),
        None,
    )
    .await?;
    let status = optional_str(args, "status");
    let agent = optional_str(args, "agent");
    let limit = integer_arg(args, "limit").unwrap_or(100).max(1) as usize;
    let filtered = tasks
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|task| {
                    status.is_none_or(|expected| {
                        task.get("status").and_then(Value::as_str) == Some(expected)
                    }) && agent.is_none_or(|expected| {
                        task.get("assigned_agent").and_then(Value::as_str) == Some(expected)
                    })
                })
                .take(limit)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(Value::Array(filtered))
}

async fn run_list(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    if let Some(id) = optional_str(args, "id") {
        let run = call(
            client,
            endpoint,
            Method::GET,
            &format!("/api/runs/{id}"),
            None,
        )
        .await?;
        if bool_arg(args, "include_timeline").unwrap_or(false) {
            let timeline = call(
                client,
                endpoint,
                Method::GET,
                &format!("/api/runs/{id}/timeline"),
                None,
            )
            .await?;
            return Ok(json!({ "run": run, "timeline": timeline }));
        }
        return Ok(run);
    }
    let mut path = ws_path("/api/runs", workspace_id);
    if let Some(task_id) = optional_str(args, "task_id") {
        path.push('&');
        path.push_str("task_id=");
        path.push_str(&urlencoding::encode(task_id));
    }
    if let Some(limit) = integer_arg(args, "limit") {
        path.push('&');
        path.push_str(&format!("limit={}", limit.max(1)));
    }
    let runs = call(client, endpoint, Method::GET, &path, None).await?;
    let status = optional_str(args, "status");
    let filtered = runs
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|run| {
                    status.is_none_or(|expected| {
                        run.get("status").and_then(Value::as_str) == Some(expected)
                    })
                })
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(Value::Array(filtered))
}

async fn run_dynamic_recipe(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    id: &str,
    args: &Value,
) -> Result<Value> {
    let trigger = args.get("trigger").cloned().unwrap_or_else(|| args.clone());
    let dry_run = bool_arg(args, "dry_run").unwrap_or(false);
    call(
        client,
        endpoint,
        Method::POST,
        &ws_path(
            &format!("/api/recipes/{}/run", urlencoding::encode(id)),
            workspace_id,
        ),
        Some(json!({
            "trigger_context": trigger,
            "dry_run": dry_run,
        })),
    )
    .await
}

async fn run_dynamic_agent(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    id: &str,
    args: &Value,
) -> Result<Value> {
    let prompt = required(args, "prompt")?;
    let title = optional(args, "title").unwrap_or_else(|| prompt.chars().take(60).collect());
    let created = call(
        client,
        endpoint,
        Method::POST,
        &ws_path("/api/tasks", workspace_id),
        Some(json!({
            "title": title,
            "description": prompt,
            "assigned_agent": id,
            "preferred_model": optional(args, "model"),
            "execution_mode": optional(args, "mode").unwrap_or_else(|| "AUTO_DSH".into()),
            "origin": "MANUAL",
        })),
    )
    .await?;
    let task_id = created
        .get("id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let mut result = json!({ "agent": id, "task": created });
    if let Some(task_id) = task_id {
        match call(
            client,
            endpoint,
            Method::POST,
            &format!("/api/tasks/{task_id}/dispatch"),
            Some(json!({ "manual_agent": id })),
        )
        .await
        {
            Ok(dispatch) => result["dispatch"] = dispatch,
            Err(error) => result["dispatch_error"] = json!(format!("{error:#}")),
        }
    }
    Ok(result)
}

async fn recipe_list(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    if let Some(id) = optional_str(args, "id") {
        let mut value = call(
            client,
            endpoint,
            Method::GET,
            &ws_path(&format!("/api/recipes/{id}"), workspace_id),
            None,
        )
        .await?;
        if !bool_arg(args, "include_yaml").unwrap_or(true)
            && let Some(object) = value.as_object_mut()
        {
            object.remove("yaml");
        }
        return Ok(value);
    }
    let value = call(
        client,
        endpoint,
        Method::GET,
        &ws_path("/api/recipes", workspace_id),
        None,
    )
    .await?;
    Ok(value.get("recipes").cloned().unwrap_or(value))
}

async fn queue_control(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    let action = required(args, "action")?;
    match action {
        "status" => {
            call(
                client,
                endpoint,
                Method::GET,
                &ws_path("/api/queue/status", workspace_id),
                None,
            )
            .await
        }
        "pause" => {
            let reason = optional_str(args, "reason").unwrap_or("pausada pelo agente");
            call(
                client,
                endpoint,
                Method::POST,
                &format!("/api/queue/pause?reason={}", urlencoding::encode(reason)),
                Some(json!({})),
            )
            .await
        }
        "resume" => {
            call(
                client,
                endpoint,
                Method::POST,
                "/api/queue/resume",
                Some(json!({})),
            )
            .await
        }
        "cancel" | "retry" => {
            let id = required(args, "job_id")?;
            let verb = if action == "cancel" {
                "cancel"
            } else {
                "retry"
            };
            call(
                client,
                endpoint,
                Method::POST,
                &format!("/api/queue/{id}/{verb}"),
                Some(json!({})),
            )
            .await
        }
        other => Err(anyhow!(
            "ação inválida '{other}'; use status, pause, resume, cancel ou retry"
        )),
    }
}

async fn artifact_list(
    client: &Client,
    endpoint: &ApiEndpoint,
    workspace_id: &str,
    args: &Value,
) -> Result<Value> {
    if let Some(id) = optional_str(args, "id") {
        return call(
            client,
            endpoint,
            Method::GET,
            &format!("/api/artifacts/{id}"),
            None,
        )
        .await;
    }
    let mut path = ws_path("/api/artifacts", workspace_id);
    for key in ["run_id", "task_id", "agent", "action"] {
        if let Some(value) = optional_str(args, key) {
            path.push('&');
            path.push_str(key);
            path.push('=');
            path.push_str(&urlencoding::encode(value));
        }
    }
    if let Some(limit) = integer_arg(args, "limit") {
        path.push('&');
        path.push_str(&format!("limit={}", limit.max(1)));
    }
    call(client, endpoint, Method::GET, &path, None).await
}

async fn ask_user(client: &Client, endpoint: &ApiEndpoint, args: &Value) -> Result<Value> {
    let task_id = required(args, "task_id")?;
    let body = json!({
        "question": required(args, "question")?,
        "kind": optional(args, "kind"),
        "context": optional(args, "context"),
        "options": args.get("options").cloned().unwrap_or_else(|| json!([])),
        "multiple": bool_arg(args, "multiple").unwrap_or(false),
        "allow_custom": bool_arg(args, "allow_custom").unwrap_or(true),
        "timeout_minutes": integer_arg(args, "timeout_minutes").unwrap_or(10),
    });
    let created = call(
        client,
        endpoint,
        Method::POST,
        &format!("/api/tasks/{task_id}/interactions"),
        Some(body),
    )
    .await?;
    let interaction_id = created
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("a API não devolveu o id da interação"))?
        .to_string();
    let timeout_minutes = integer_arg(args, "timeout_minutes")
        .unwrap_or(10)
        .clamp(1, 120) as u64;
    let started = Instant::now();
    loop {
        let current = call(
            client,
            endpoint,
            Method::GET,
            &format!("/api/interactions/{interaction_id}"),
            None,
        )
        .await?;
        let status = current
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("PENDING");
        if status != "PENDING" {
            let answer = current
                .get("answer_json")
                .and_then(Value::as_str)
                .and_then(|raw| serde_json::from_str::<Value>(raw).ok());
            return Ok(json!({
                "interaction_id": interaction_id,
                "status": status,
                "answer": answer,
            }));
        }
        if started.elapsed() >= Duration::from_secs(timeout_minutes * 60) {
            return Ok(json!({
                "interaction_id": interaction_id,
                "status": "TIMEOUT",
                "message": "O humano não respondeu dentro do prazo. Não assuma dados críticos; bloqueie a tarefa ou peça instruções."
            }));
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
