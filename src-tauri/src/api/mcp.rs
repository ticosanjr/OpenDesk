use crate::{app_state::AppState, error::AppError, error::AppResult, mcp_session::McpEvent};
use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use futures_util::{Stream, StreamExt, stream};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{convert::Infallible, sync::Arc, time::Duration};
use tokio::sync::broadcast;

#[derive(Debug, Deserialize, Default)]
pub struct McpQuery {
    pub workspace_id: Option<String>,
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

fn with_session(mut response: Response, session_id: &str) -> Response {
    if let Ok(value) = HeaderValue::from_str(session_id) {
        response.headers_mut().insert("mcp-session-id", value);
    }
    response
}

fn sse_event(event: McpEvent) -> Result<Event, Infallible> {
    Ok(Event::default()
        .id(event.id.to_string())
        .event("message")
        .data(event.payload.to_string()))
}

/// JSON-RPC over Streamable HTTP.
///
/// A session is created on `initialize`, returned in `Mcp-Session-Id` and
/// reused by later requests. Requests with `Accept: text/event-stream` get a
/// single SSE event; notifications are acknowledged with 202.
pub async fn rpc(
    State(state): State<Arc<AppState>>,
    Query(query): Query<McpQuery>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> AppResult<Response> {
    let workspace_id =
        crate::api::tasks::resolve_workspace_id(&state, query.workspace_id.as_deref()).await?;
    let header_agent = header_value(&headers, "x-opendesk-agent").unwrap_or_else(|| "build".into());
    let header_phase =
        header_value(&headers, "x-opendesk-mcp-phase").unwrap_or_else(|| "ALL".into());
    let header_role = header_value(&headers, "x-opendesk-mcp-role").unwrap_or_default();
    let requested_session = header_value(&headers, "mcp-session-id");
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");

    // JSON-RPC responses from the client (elicitation/sampling) are routed to
    // the waiting server request instead of the tool dispatcher.
    if request.get("method").is_none()
        && (request.get("result").is_some() || request.get("error").is_some())
    {
        if let Some(id) = requested_session.as_deref()
            && let Some(session) = state.mcp_sessions.get(id)
        {
            session.resolve_client_response(&request);
        }
        return Ok(StatusCode::ACCEPTED.into_response());
    }

    let session = if let Some(id) = requested_session.as_deref() {
        let session = state
            .mcp_sessions
            .get(id)
            .ok_or_else(|| AppError::NotFound(format!("sessão MCP {id}")))?;
        if session.workspace_id != workspace_id {
            return Err(AppError::BadRequest(
                "sessão MCP pertence a outro workspace".into(),
            ));
        }
        Some(session)
    } else if method == "initialize" {
        Some(
            state
                .mcp_sessions
                .create(&workspace_id, &header_agent, &header_phase, &header_role),
        )
    } else {
        None
    };

    let (agent, phase, role) = match &session {
        Some(session) => (
            session.agent.as_str(),
            session.phase.as_str(),
            session.role.as_str(),
        ),
        None => (
            header_agent.as_str(),
            header_phase.as_str(),
            header_role.as_str(),
        ),
    };
    let workspace = crate::db::workspaces::get(&state.db, &workspace_id).await?;
    let workspace_root = std::path::PathBuf::from(workspace.root_path);
    let policy = crate::db::settings::get(&state.db, "mcp_agent_tools")
        .await
        .ok()
        .flatten()
        .and_then(|config| crate::mcp_rbac::policy_for_agent(&config, agent));

    // Notifications do not produce a JSON-RPC response.
    if request.get("id").is_none() {
        let _ = crate::mcp_control::handle_http_with(
            &workspace_id,
            agent,
            phase,
            role,
            policy.as_ref(),
            Some(&state.mcp_pool),
            Some(&workspace_root),
            request,
        )
        .await;
        let mut response = StatusCode::ACCEPTED.into_response();
        if let Some(session) = &session {
            response = with_session(response, &session.id);
        }
        return Ok(response);
    }

    let tool = request
        .pointer("/params/name")
        .and_then(Value::as_str)
        .map(str::to_string);
    let alvo = tool.as_deref().unwrap_or("");
    let is_call = method == "tools/call";
    if is_call
        && let Some(name) = &tool
        && policy.as_ref().is_some_and(|policy| !policy.allows(name))
    {
        let _ = crate::audit::record_mcp_tool(&state.db, &workspace_id, agent, name, 403).await;
        let response = json!({
            "jsonrpc": "2.0",
            "id": request.get("id").cloned().unwrap_or(Value::Null),
            "error": { "code": -32001, "message": format!("agente sem permissão para a tool {name}") }
        });
        let mut response = Json(response).into_response();
        if let Some(session) = &session {
            response = with_session(response, &session.id);
        }
        return Ok(response);
    }
    if is_call {
        let config = crate::db::settings::get(&state.db, "mcp_tool_rates")
            .await
            .ok()
            .flatten()
            .and_then(|value| serde_json::from_value::<crate::mcp_rate::McpRateConfig>(value).ok())
            .unwrap_or_default();
        let limit = config.limit_for(agent, alvo);
        if let Err(retry) = state.mcp_rate.check(agent, alvo, limit) {
            let _ = crate::audit::record_mcp_tool(&state.db, &workspace_id, agent, alvo, 429).await;
            let response = json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(Value::Null),
                "error": {
                    "code": -32002,
                    "message": format!("rate limit da tool {alvo} excedido; tente em {retry}s")
                }
            });
            let mut response = Json(response).into_response();
            if let Some(session) = &session {
                response = with_session(response, &session.id);
            }
            return Ok(response);
        }
    }
    if is_call {
        let arguments = request
            .pointer("/params/arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let timeout = arguments
            .get("timeout_seconds")
            .and_then(Value::as_u64)
            .unwrap_or(120)
            .clamp(5, 300);
        if alvo == "elicit" {
            let message = arguments
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Informe os dados solicitados.");
            let schema = arguments
                .get("schema")
                .cloned()
                .unwrap_or_else(|| json!({ "type": "object", "properties": {} }));
            let result = if let Some(session) = &session {
                session
                    .request(
                        "elicitation/create",
                        json!({
                            "message": message,
                            "requestedSchema": schema,
                        }),
                        Duration::from_secs(timeout),
                    )
                    .await
            } else {
                None
            };
            let result = match result {
                Some(response) => response
                    .get("result")
                    .cloned()
                    .or_else(|| response.get("error").cloned()),
                None if arguments.get("task_id").is_some() => {
                    crate::mcp_control::elicit_fallback(&arguments).await.ok()
                }
                None => None,
            };
            let _ = crate::audit::record_mcp_tool(
                &state.db,
                &workspace_id,
                agent,
                "elicit",
                if result.is_some() { 200 } else { 408 },
            )
            .await;
            let value = match result {
                Some(result) => json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id").cloned().unwrap_or(Value::Null),
                    "result": {
                        "content": [{ "type": "text", "text": result.to_string() }],
                        "isError": false,
                    }
                }),
                None => json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id").cloned().unwrap_or(Value::Null),
                    "error": { "code": -32003, "message": "elicitation sem resposta; informe task_id para o fallback HITL" }
                }),
            };
            let mut response = Json(value).into_response();
            if let Some(session) = &session {
                response = with_session(response, &session.id);
            }
            return Ok(response);
        }
        if alvo == "sampling" {
            let prompt = arguments
                .get("prompt")
                .and_then(Value::as_str)
                .unwrap_or("");
            let remote = if let Some(session) = &session {
                session
                    .request(
                        "sampling/createMessage",
                        json!({
                            "messages": [{ "role": "user", "content": { "type": "text", "text": prompt } }],
                            "maxTokens": arguments.get("max_tokens").and_then(Value::as_u64).unwrap_or(1024),
                        }),
                        Duration::from_secs(timeout),
                    )
                    .await
            } else {
                None
            };
            let value = match remote {
                Some(response) => json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id").cloned().unwrap_or(Value::Null),
                    "result": response.get("result").cloned().unwrap_or(response),
                }),
                None => {
                    match crate::mcp_sampling::fallback(&state, &workspace_id, &arguments).await {
                        Ok(sampled) => json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id").cloned().unwrap_or(Value::Null),
                            "result": {
                                "content": [{ "type": "text", "text": sampled.to_string() }],
                                "isError": false,
                            }
                        }),
                        Err(error) => json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id").cloned().unwrap_or(Value::Null),
                            "error": { "code": -32004, "message": format!("{error:#}") }
                        }),
                    }
                }
            };
            let _ = crate::audit::record_mcp_tool(
                &state.db,
                &workspace_id,
                agent,
                "sampling",
                if value.get("error").is_some() {
                    424
                } else {
                    200
                },
            )
            .await;
            let mut response = Json(value).into_response();
            if let Some(session) = &session {
                response = with_session(response, &session.id);
            }
            return Ok(response);
        }
    }

    let value = crate::mcp_control::handle_http_with(
        &workspace_id,
        agent,
        phase,
        role,
        policy.as_ref(),
        Some(&state.mcp_pool),
        Some(&workspace_root),
        request,
    )
    .await;
    if is_call {
        let _ = crate::audit::record_mcp_tool(&state.db, &workspace_id, agent, alvo, 200).await;
    }

    let wants_sse =
        header_value(&headers, "accept").is_some_and(|value| value.contains("text/event-stream"));
    if wants_sse {
        let event = Event::default().event("message").data(value.to_string());
        let mut response = Sse::new(stream::iter(vec![Ok::<_, Infallible>(event)]))
            .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
            .into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/event-stream"),
        );
        if let Some(session) = &session {
            response = with_session(response, &session.id);
        }
        return Ok(response);
    }

    let mut response = Json(value).into_response();
    if let Some(session) = &session {
        response = with_session(response, &session.id);
    }
    Ok(response)
}

/// Server-to-client stream used for notifications and requests (elicitation,
/// sampling). Reconnects replay events after `Last-Event-ID`.
pub async fn stream(
    State(state): State<Arc<AppState>>,
    Query(query): Query<McpQuery>,
    headers: HeaderMap,
) -> AppResult<Sse<impl Stream<Item = Result<Event, Infallible>>>> {
    let _ = query;
    let id = header_value(&headers, "mcp-session-id")
        .ok_or_else(|| AppError::BadRequest("Mcp-Session-Id é obrigatório no GET".into()))?;
    let session = state
        .mcp_sessions
        .get(&id)
        .ok_or_else(|| AppError::NotFound(format!("sessão MCP {id}")))?;
    let last = header_value(&headers, "last-event-id").and_then(|value| value.parse().ok());
    let (replay, receiver) = session.subscribe(last);
    let replay = stream::iter(replay.into_iter().map(sse_event));
    let live = stream::unfold(receiver, |mut receiver| async move {
        loop {
            match receiver.recv().await {
                Ok(event) => return Some((sse_event(event), receiver)),
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    });
    Ok(Sse::new(replay.chain(live)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    ))
}

/// Terminates a Streamable HTTP session.
pub async fn delete(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> AppResult<StatusCode> {
    let id = header_value(&headers, "mcp-session-id")
        .ok_or_else(|| AppError::BadRequest("Mcp-Session-Id é obrigatório no DELETE".into()))?;
    state.mcp_sessions.remove(&id);
    Ok(StatusCode::NO_CONTENT)
}
