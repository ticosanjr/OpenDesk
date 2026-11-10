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
use anyhow::{Context, Result, anyhow};
use reqwest::{Client, Method};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub async fn run_stdio() -> Result<()> {
    let workspace_id = std::env::var("OPENDESK_WORKSPACE_ID")
        .context("OPENDESK_WORKSPACE_ID é obrigatório para o MCP de controle")?;
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut lines = BufReader::new(stdin).lines();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("OpenDesk MCP control: invalid JSON: {error}");
                continue;
            }
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let response = match method {
            "initialize" => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": {
                        "name": "opendesk-control",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            })),
            "notifications/initialized" => None,
            "ping" => Some(json!({ "jsonrpc": "2.0", "id": id, "result": {} })),
            "tools/list" => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "tools": tools() }
            })),
            "tools/call" => {
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let arguments = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                match dispatch(&workspace_id, name, arguments).await {
                    Ok(value) => Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": value.to_string() }],
                            "isError": false
                        }
                    })),
                    Err(error) => Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": format!("{error:#}") }],
                            "isError": true
                        }
                    })),
                }
            }
            "shutdown" => Some(json!({ "jsonrpc": "2.0", "id": id, "result": null })),
            "exit" => break,
            _ => Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("method not found: {method}") }
            })),
        };
        if let Some(response) = response {
            stdout
                .write_all(serde_json::to_string(&response)?.as_bytes())
                .await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }
    }
    Ok(())
}

async fn dispatch(workspace_id: &str, tool: &str, args: Value) -> Result<Value> {
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
                "position_index": args.get("position_index").cloned(),
            });
            call(
                &client,
                &endpoint,
                Method::POST,
                &format!("/api/tasks/{id}/move"),
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

fn task_create_body(args: &Value) -> Result<Value> {
    Ok(json!({
        "title": required(args, "title")?,
        "description": required(args, "description")?,
        "execution_mode": optional(args, "execution_mode").unwrap_or_else(|| "AUTO_DSH".into()),
        "priority": integer_arg(args, "priority"),
        "assigned_agent": optional(args, "assigned_agent"),
        "preferred_model": optional(args, "preferred_model"),
        "effort_level": optional(args, "effort_level"),
        "notify_email": optional(args, "notify_email"),
        "export_to_gdocs": bool_arg(args, "export_to_gdocs").unwrap_or(false),
        "tags": string_array(args, "tags"),
        "origin": "MANUAL",
    }))
}

fn task_update_body(args: &Value) -> Result<Value> {
    required(args, "id")?;
    let mut body = serde_json::Map::new();
    for key in [
        "title",
        "description",
        "assigned_agent",
        "preferred_model",
        "execution_mode",
        "effort_level",
        "notify_email",
        "priority",
    ] {
        if let Some(value) = args.get(key).filter(|value| !value.is_null()) {
            body.insert(key.into(), value.clone());
        }
    }
    if let Some(tags) = args.get("tags") {
        body.insert("tags".into(), tags.clone());
    }
    for key in ["export_to_gdocs", "clear_agent", "clear_model"] {
        if let Some(value) = args.get(key).and_then(Value::as_bool) {
            body.insert(key.into(), json!(value));
        }
    }
    Ok(Value::Object(body))
}

fn routine_body(workspace_id: &str, args: &Value) -> Result<Value> {
    let recipe_id = optional_str(args, "recipe_id");
    let target_kind = optional_str(args, "target_kind")
        .map(str::to_string)
        .unwrap_or_else(|| {
            if recipe_id.is_some() {
                "RECIPE".into()
            } else {
                "PROMPT".into()
            }
        });
    let recurrence = args
        .get("recurrence")
        .filter(|value| !value.is_null())
        .map(serde_json::to_string)
        .transpose()?;
    let cron_expression = optional_str(args, "cron_expression")
        .map(str::to_string)
        .unwrap_or_default();
    if recurrence.is_none() && cron_expression.trim().is_empty() {
        return Err(anyhow!(
            "informe 'recurrence' (preferido) ou 'cron_expression'"
        ));
    }
    let prompt = optional_str(args, "command_or_prompt")
        .map(str::to_string)
        .unwrap_or_default();
    if target_kind == "PROMPT" && prompt.trim().is_empty() {
        return Err(anyhow!(
            "command_or_prompt é obrigatório quando target_kind=PROMPT"
        ));
    }
    Ok(json!({
        "workspace_id": workspace_id,
        "name": required(args, "name")?,
        "description": optional(args, "description"),
        "cron_expression": cron_expression,
        "timezone": optional(args, "timezone")
            .unwrap_or_else(|| iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".into())),
        "agent_name": optional(args, "agent_name"),
        "command_or_prompt": prompt,
        "trigger_type": optional(args, "trigger_type").unwrap_or_else(|| "CRON".into()),
        "trigger_config_json": optional(args, "trigger_config_json").unwrap_or_else(|| "{}".into()),
        "target_kind": target_kind,
        "recipe_id": recipe_id,
        "router_policy": optional(args, "router_policy").unwrap_or_else(|| "COST_BENEFIT".into()),
        "output_folder_template": optional(args, "output_folder_template")
            .unwrap_or_else(|| "output/{YYYY}-{MM}/{schedule_slug}".into()),
        "notify_email_recipients": optional(args, "notify_email_recipients"),
        "notify_signal": bool_arg(args, "notify_signal").unwrap_or(false),
        "notify_signal_recipients": optional(args, "notify_signal_recipients"),
        "notify_only_on_change": bool_arg(args, "notify_only_on_change").unwrap_or(false),
        "recurrence_json": recurrence,
        "one_time": false,
        "misfire_policy": optional(args, "misfire_policy").unwrap_or_else(|| "SKIP_TO_NEXT".into()),
        "safe_readonly_mode": bool_arg(args, "safe_readonly_mode").unwrap_or(true),
        "sync_to_google_docs": bool_arg(args, "sync_to_google_docs").unwrap_or(false),
        "sync_to_google_calendar": bool_arg(args, "sync_to_google_calendar").unwrap_or(false),
        // The backend also forces this; sending it makes the intent explicit.
        "is_active": false,
    }))
}

fn routine_update_body(args: &Value) -> Result<Value> {
    required(args, "id")?;
    let mut body = serde_json::Map::new();
    for key in [
        "name",
        "description",
        "cron_expression",
        "timezone",
        "agent_name",
        "command_or_prompt",
        "trigger_type",
        "trigger_config_json",
        "target_kind",
        "recipe_id",
        "router_policy",
        "output_folder_template",
        "notify_email_recipients",
        "notify_signal_recipients",
        "misfire_policy",
    ] {
        if let Some(value) = args.get(key).filter(|value| !value.is_null()) {
            body.insert(key.into(), value.clone());
        }
    }
    for key in [
        "notify_signal",
        "notify_only_on_change",
        "safe_readonly_mode",
        "sync_to_google_docs",
        "sync_to_google_calendar",
    ] {
        if let Some(value) = args.get(key).and_then(Value::as_bool) {
            body.insert(key.into(), json!(value));
        }
    }
    if let Some(recurrence) = args.get("recurrence").filter(|value| !value.is_null()) {
        body.insert(
            "recurrence_json".into(),
            json!(serde_json::to_string(recurrence)?),
        );
    }
    Ok(Value::Object(body))
}

fn ws_path(path: &str, workspace_id: &str) -> String {
    format!("{path}?workspace_id={}", urlencoding::encode(workspace_id))
}

fn required<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    optional_str(args, key).ok_or_else(|| anyhow!("argumento '{key}' é obrigatório"))
}

fn optional(args: &Value, key: &str) -> Option<String> {
    optional_str(args, key).map(str::to_string)
}

fn optional_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn bool_arg(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(Value::as_bool)
}

fn integer_arg(args: &Value, key: &str) -> Option<i64> {
    args.get(key).and_then(|value| {
        value
            .as_i64()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
    })
}

fn string_array(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

fn empty() -> Value {
    json!({ "type": "object", "properties": {} })
}

fn tools() -> Vec<Value> {
    vec![
        tool(
            "opendesk_status",
            "Resumo operacional: workspace, motor OpenCode, fila, budget e itens que precisam de atenção. Não resolve HITL, permissões ou propostas.",
            empty(),
        ),
        tool(
            "task_list",
            "Lista tarefas do workspace, com filtros simples. Passe 'id' para abrir o detalhe de uma tarefa.",
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "status": { "type": "string", "enum": ["TODO", "IN_PROGRESS", "IN_REVIEW", "BLOCKED", "DONE", "CANCELLED"] },
                    "agent": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1 }
                }
            }),
        ),
        tool(
            "task_create",
            "Cria uma tarefa no Kanban em TODO. Use para trabalho durável, não para passos triviais.",
            json!({
                "type": "object",
                "required": ["title", "description"],
                "properties": {
                    "title": { "type": "string" },
                    "description": { "type": "string" },
                    "assigned_agent": { "type": "string" },
                    "preferred_model": { "type": "string" },
                    "execution_mode": { "type": "string" },
                    "effort_level": { "type": "string" },
                    "priority": { "type": "integer", "minimum": 1, "maximum": 4 },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "notify_email": { "type": "string" },
                    "export_to_gdocs": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "task_update",
            "Atualiza campos de uma tarefa. Mudanças de coluna usam task_move.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "title": { "type": "string" },
                    "description": { "type": "string" },
                    "assigned_agent": { "type": "string" },
                    "preferred_model": { "type": "string" },
                    "execution_mode": { "type": "string" },
                    "effort_level": { "type": "string" },
                    "priority": { "type": "integer", "minimum": 1, "maximum": 4 },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "notify_email": { "type": "string" },
                    "export_to_gdocs": { "type": "boolean" },
                    "clear_agent": { "type": "boolean" },
                    "clear_model": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "task_move",
            "Move a tarefa para outra coluna/status do Kanban, validando a transição no backend.",
            json!({
                "type": "object",
                "required": ["id", "status"],
                "properties": {
                    "id": { "type": "string" },
                    "status": { "type": "string", "enum": ["TODO", "IN_PROGRESS", "IN_REVIEW", "BLOCKED", "DONE", "CANCELLED"] },
                    "position_index": { "type": "integer" }
                }
            }),
        ),
        tool(
            "task_dispatch",
            "Enfileira uma tarefa para execução no OpenCode. Exige motor online e respeita budget e roteador.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "policy": { "type": "string" },
                    "manual_agent": { "type": "string" },
                    "manual_model": { "type": "string" },
                    "manual_effort": { "type": "string" },
                    "execution_mode": { "type": "string" }
                }
            }),
        ),
        tool(
            "run_list",
            "Lista execuções ou abre uma execução. Com include_timeline=true e id, devolve run + timeline.",
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "task_id": { "type": "string" },
                    "status": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1 },
                    "include_timeline": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "routine_list",
            "Lista rotinas do workspace (ativas e pausadas) ou abre uma rotina por id.",
            json!({
                "type": "object",
                "properties": { "id": { "type": "string" } }
            }),
        ),
        tool(
            "routine_create",
            "Cria uma rotina SEMPRE desligada. Informe recurrence (preferido) ou cron_expression. Ativar é uma ação humana explícita.",
            json!({
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "recurrence": {
                        "type": "object",
                        "properties": {
                            "frequency": { "type": "string", "enum": ["once", "hourly", "daily", "weekly", "weekdays", "monthly", "custom"] },
                            "interval": { "type": "integer", "minimum": 1 },
                            "time": { "type": "string" },
                            "weekdays": { "type": "array", "items": { "type": "integer", "minimum": 1, "maximum": 7 } },
                            "day_of_month": { "type": "integer", "minimum": 1, "maximum": 31 },
                            "start_date": { "type": "string" },
                            "end_date": { "type": "string" },
                            "cron_expression": { "type": "string" }
                        }
                    },
                    "cron_expression": { "type": "string" },
                    "timezone": { "type": "string" },
                    "trigger_type": { "type": "string" },
                    "target_kind": { "type": "string", "enum": ["PROMPT", "RECIPE"] },
                    "recipe_id": { "type": "string" },
                    "command_or_prompt": { "type": "string" },
                    "agent_name": { "type": "string" },
                    "router_policy": { "type": "string" },
                    "output_folder_template": { "type": "string" },
                    "notify_email_recipients": { "type": "string" },
                    "notify_signal": { "type": "boolean" },
                    "notify_signal_recipients": { "type": "string" },
                    "notify_only_on_change": { "type": "boolean" },
                    "misfire_policy": { "type": "string" },
                    "safe_readonly_mode": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "routine_update",
            "Edita uma rotina existente. Não altera o estado ligado/desligado; use routine_set_active.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "name": { "type": "string" },
                    "description": { "type": "string" },
                    "recurrence": { "type": "object" },
                    "cron_expression": { "type": "string" },
                    "timezone": { "type": "string" },
                    "recipe_id": { "type": "string" },
                    "command_or_prompt": { "type": "string" },
                    "agent_name": { "type": "string" },
                    "router_policy": { "type": "string" },
                    "output_folder_template": { "type": "string" },
                    "notify_email_recipients": { "type": "string" },
                    "notify_signal": { "type": "boolean" },
                    "notify_signal_recipients": { "type": "string" },
                    "notify_only_on_change": { "type": "boolean" },
                    "misfire_policy": { "type": "string" },
                    "safe_readonly_mode": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "routine_set_active",
            "Liga ou desliga uma rotina. Só use quando o usuário pedir explicitamente; nunca ative por conta própria.",
            json!({
                "type": "object",
                "required": ["id", "active"],
                "properties": {
                    "id": { "type": "string" },
                    "active": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "routine_run",
            "Executa uma rotina agora. Use dry_run=true para validar sem executar.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "dry_run": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "recipe_list",
            "Lista receitas ou abre uma receita por id. Com id, include_yaml=false omite o YAML.",
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "include_yaml": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "recipe_save",
            "Cria ou atualiza uma receita YAML. Receitas novas são gravadas desligadas; edições preservam o estado.",
            json!({
                "type": "object",
                "required": ["id", "yaml"],
                "properties": {
                    "id": { "type": "string" },
                    "yaml": { "type": "string" }
                }
            }),
        ),
        tool(
            "recipe_set_enabled",
            "Liga ou desliga uma receita. Só use quando o usuário pedir explicitamente; nunca ligue por conta própria.",
            json!({
                "type": "object",
                "required": ["id", "enabled"],
                "properties": {
                    "id": { "type": "string" },
                    "enabled": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "recipe_run",
            "Executa uma receita agora. Use dry_run=true para validar sem executar.",
            json!({
                "type": "object",
                "required": ["id"],
                "properties": {
                    "id": { "type": "string" },
                    "trigger_context": { "type": "object" },
                    "dry_run": { "type": "boolean" }
                }
            }),
        ),
        tool(
            "attention_list",
            "Lista HITL, permissões, propostas e falhas que precisam de decisão humana. Apenas leitura: nunca aprove por conta própria.",
            empty(),
        ),
        tool(
            "queue_control",
            "Consulta ou controla a fila: status, pause, resume, cancel ou retry (cancel/retry exigem job_id).",
            json!({
                "type": "object",
                "required": ["action"],
                "properties": {
                    "action": { "type": "string", "enum": ["status", "pause", "resume", "cancel", "retry"] },
                    "job_id": { "type": "string" },
                    "reason": { "type": "string" }
                }
            }),
        ),
        tool(
            "artifact_list",
            "Lista artefatos do workspace ou abre um artefato por id, com filtros por run, tarefa, agente e ação.",
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "run_id": { "type": "string" },
                    "task_id": { "type": "string" },
                    "agent": { "type": "string" },
                    "action": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1 }
                }
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_the_expected_control_tools() {
        let names = tools()
            .into_iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
            .collect::<Vec<_>>();
        for expected in [
            "opendesk_status",
            "task_list",
            "task_create",
            "task_update",
            "task_move",
            "task_dispatch",
            "run_list",
            "routine_list",
            "routine_create",
            "routine_update",
            "routine_set_active",
            "routine_run",
            "recipe_list",
            "recipe_save",
            "recipe_set_enabled",
            "recipe_run",
            "attention_list",
            "queue_control",
            "artifact_list",
        ] {
            assert!(names.iter().any(|name| name == expected), "{expected}");
        }
        assert_eq!(names.len(), 19);
    }

    #[test]
    fn task_create_body_has_safe_defaults() {
        let body = task_create_body(&json!({
            "title": "Relatório",
            "description": "Produza o relatório mensal"
        }))
        .expect("body");
        assert_eq!(body["execution_mode"], "AUTO_DSH");
        assert_eq!(body["origin"], "MANUAL");
        assert_eq!(body["export_to_gdocs"], false);
        assert!(body["tags"].as_array().unwrap().is_empty());
    }

    #[test]
    fn routine_body_always_starts_inactive() {
        let body = routine_body(
            "ws-1",
            &json!({
                "name": "Resumo diário",
                "recurrence": { "frequency": "daily", "time": "08:00" },
                "command_or_prompt": "Resuma o dia"
            }),
        )
        .expect("body");
        assert_eq!(body["is_active"], false);
        assert!(body["recurrence_json"].as_str().unwrap().contains("daily"));
        assert_eq!(body["target_kind"], "PROMPT");
    }

    #[test]
    fn routine_body_requires_a_schedule() {
        let error = routine_body("ws-1", &json!({ "name": "Sem agenda" })).unwrap_err();
        assert!(error.to_string().contains("recurrence"));
    }

    #[test]
    fn required_argument_is_reported() {
        let error = required(&json!({}), "id").unwrap_err();
        assert!(error.to_string().contains("id"));
    }
}
