use crate::{
    db,
    events::EventBus,
    integrations::{google_api::GoogleApi, google_oauth::GoogleOAuth},
    paths::AppPaths,
    secrets::SecretStore,
};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::{path::PathBuf, sync::Arc};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub async fn run_stdio() -> Result<()> {
    let workspace_id = std::env::var("OPENDESK_WORKSPACE_ID")
        .context("OPENDESK_WORKSPACE_ID is required for the Google MCP bridge")?;
    let paths = AppPaths::discover()?;
    let pool = db::init(&paths.db_path).await?;
    let secrets = Arc::new(SecretStore::new(&paths));
    let events = EventBus::new(64);
    let google = GoogleOAuth::new(pool, secrets, events);
    let api = GoogleApi::new(google);

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
                eprintln!("OpenDesk MCP: invalid JSON: {error}");
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
                        "name": "opendesk-google",
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
                match call_tool(&api, &workspace_id, name, &arguments).await {
                    Ok(text) => Some(json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "content": [{ "type": "text", "text": text }],
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

async fn call_tool(
    api: &GoogleApi,
    workspace_id: &str,
    name: &str,
    arguments: &Value,
) -> Result<String> {
    match name {
        "gmail_search_threads" => {
            let query = string_arg(arguments, "query").unwrap_or_default();
            Ok(api.gmail_search(workspace_id, &query).await?.to_string())
        }
        "gmail_create_draft" => {
            let to = required_arg(arguments, "to")?;
            let subject = required_arg(arguments, "subject")?;
            let body = string_arg(arguments, "body").unwrap_or_default();
            Ok(api
                .gmail_create_draft(workspace_id, &to, &subject, &body)
                .await?
                .to_string())
        }
        "gmail_send_email" => {
            let to = required_arg(arguments, "to")?;
            let subject = required_arg(arguments, "subject")?;
            let body = string_arg(arguments, "body").unwrap_or_default();
            let mut attachments = Vec::new();
            if let Some(paths) = arguments.get("attachments").and_then(Value::as_array) {
                for path in paths.iter().filter_map(Value::as_str) {
                    let path = PathBuf::from(path);
                    let bytes = std::fs::read(&path)
                        .with_context(|| format!("could not read attachment {}", path.display()))?;
                    attachments.push((
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("attachment")
                            .to_string(),
                        bytes,
                        mime_for(&path).to_string(),
                    ));
                }
            }
            Ok(api
                .gmail_send(workspace_id, &to, &subject, &body, attachments)
                .await?
                .to_string())
        }
        "gdocs_read_document" => {
            let document_id = required_arg(arguments, "document_id")?;
            Ok(api.gdocs_read(workspace_id, &document_id).await?)
        }
        "gdocs_create_from_markdown" => {
            let title = required_arg(arguments, "title")?;
            let markdown = required_arg(arguments, "markdown")?;
            let folder_id = string_arg(arguments, "folder_id");
            Ok(api
                .gdocs_create_from_markdown(
                    workspace_id,
                    &title,
                    &markdown,
                    folder_id.as_deref(),
                )
                .await?
                .to_string())
        }
        "gdocs_append_text" => {
            let document_id = required_arg(arguments, "document_id")?;
            let text = required_arg(arguments, "text")?;
            Ok(api
                .gdocs_append_text(workspace_id, &document_id, &text)
                .await?
                .to_string())
        }
        "gdrive_search_files" => {
            let query = string_arg(arguments, "query")
                .unwrap_or_else(|| "trashed = false".to_string());
            Ok(api.drive_search(workspace_id, &query).await?.to_string())
        }
        "gdrive_upload_file" => {
            let path = PathBuf::from(required_arg(arguments, "path")?);
            let name = string_arg(arguments, "name").unwrap_or_else(|| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("artifact")
                    .to_string()
            });
            let folder_id = string_arg(arguments, "folder_id");
            let bytes = std::fs::read(&path)
                .with_context(|| format!("could not read {}", path.display()))?;
            Ok(api
                .drive_upload(
                    workspace_id,
                    &name,
                    mime_for(&path),
                    bytes,
                    folder_id.as_deref(),
                    false,
                )
                .await?
                .to_string())
        }
        "gdrive_create_folder" => {
            let name = required_arg(arguments, "name")?;
            let parent_id = string_arg(arguments, "parent_id");
            Ok(api
                .drive_create_folder(workspace_id, &name, parent_id.as_deref())
                .await?
                .to_string())
        }
        "gcalendar_list_events" => {
            let time_min = string_arg(arguments, "time_min");
            let time_max = string_arg(arguments, "time_max");
            Ok(api
                .calendar_list(workspace_id, time_min.as_deref(), time_max.as_deref())
                .await?
                .to_string())
        }
        "gcalendar_create_event" => {
            let summary = required_arg(arguments, "summary")?;
            let start = required_arg(arguments, "start")?;
            let end = required_arg(arguments, "end")?;
            let description = string_arg(arguments, "description");
            let timezone = string_arg(arguments, "timezone");
            Ok(api
                .calendar_create(
                    workspace_id,
                    &summary,
                    description.as_deref(),
                    &start,
                    &end,
                    timezone.as_deref(),
                )
                .await?
                .to_string())
        }
        "gtasks_list" => Ok(api.tasks_list(workspace_id).await?.to_string()),
        "gtasks_create" => {
            let title = required_arg(arguments, "title")?;
            let notes = string_arg(arguments, "notes");
            Ok(api
                .tasks_create(workspace_id, &title, notes.as_deref())
                .await?
                .to_string())
        }
        "gkeep_list_notes" | "gkeep_create_note" => Ok(
            "Google Keep API requires a Google Workspace enterprise account and service-account access. Use gtasks_list/gtasks_create for the backlog.".into(),
        ),
        _ => anyhow::bail!("unknown tool: {name}"),
    }
}

fn tools() -> Vec<Value> {
    let empty = json!({ "type": "object", "properties": {} });
    vec![
        tool(
            "gmail_search_threads",
            "Search Gmail threads.",
            json!({
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "required": ["query"]
            }),
        ),
        tool(
            "gmail_create_draft",
            "Create a Gmail draft.",
            json!({
                "type": "object",
                "properties": {
                    "to": { "type": "string" },
                    "subject": { "type": "string" },
                    "body": { "type": "string" }
                },
                "required": ["to", "subject", "body"]
            }),
        ),
        tool(
            "gmail_send_email",
            "Send an email through Gmail.",
            json!({
                "type": "object",
                "properties": {
                    "to": { "type": "string" },
                    "subject": { "type": "string" },
                    "body": { "type": "string" },
                    "attachments": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["to", "subject", "body"]
            }),
        ),
        tool(
            "gdocs_read_document",
            "Read the text of a Google Doc.",
            json!({
                "type": "object",
                "properties": { "document_id": { "type": "string" } },
                "required": ["document_id"]
            }),
        ),
        tool(
            "gdocs_create_from_markdown",
            "Create a Google Doc from Markdown.",
            json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "markdown": { "type": "string" },
                    "folder_id": { "type": "string" }
                },
                "required": ["title", "markdown"]
            }),
        ),
        tool(
            "gdocs_append_text",
            "Append text to a Google Doc.",
            json!({
                "type": "object",
                "properties": {
                    "document_id": { "type": "string" },
                    "text": { "type": "string" }
                },
                "required": ["document_id", "text"]
            }),
        ),
        tool(
            "gdrive_search_files",
            "Search Google Drive files.",
            json!({
                "type": "object",
                "properties": { "query": { "type": "string" } },
                "required": ["query"]
            }),
        ),
        tool(
            "gdrive_upload_file",
            "Upload a local file to Google Drive.",
            json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "name": { "type": "string" },
                    "folder_id": { "type": "string" }
                },
                "required": ["path"]
            }),
        ),
        tool(
            "gdrive_create_folder",
            "Create a Google Drive folder.",
            json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "parent_id": { "type": "string" }
                },
                "required": ["name"]
            }),
        ),
        tool(
            "gcalendar_list_events",
            "List Google Calendar events.",
            json!({
                "type": "object",
                "properties": {
                    "time_min": { "type": "string" },
                    "time_max": { "type": "string" }
                }
            }),
        ),
        tool(
            "gcalendar_create_event",
            "Create a Google Calendar event.",
            json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string" },
                    "description": { "type": "string" },
                    "start": { "type": "string" },
                    "end": { "type": "string" },
                    "timezone": { "type": "string" }
                },
                "required": ["summary", "start", "end"]
            }),
        ),
        tool("gtasks_list", "List Google Tasks.", empty.clone()),
        tool(
            "gtasks_create",
            "Create a Google Task.",
            json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "notes": { "type": "string" }
                },
                "required": ["title"]
            }),
        ),
        tool(
            "gkeep_list_notes",
            "Google Keep is enterprise-only; use gtasks_list.",
            empty.clone(),
        ),
        tool(
            "gkeep_create_note",
            "Google Keep is enterprise-only; use gtasks_create.",
            empty,
        ),
    ]
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

fn string_arg(arguments: &Value, key: &str) -> Option<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn required_arg(arguments: &Value, key: &str) -> Result<String> {
    string_arg(arguments, key).with_context(|| format!("missing required argument '{key}'"))
}

fn mime_for(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("md") | Some("markdown") => "text/markdown",
        Some("json") => "application/json",
        Some("html") | Some("htm") => "text/html",
        Some("txt") | Some("log") => "text/plain",
        Some("csv") => "text/csv",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_the_expected_tools() {
        let names = tools()
            .into_iter()
            .filter_map(|tool| tool.get("name").and_then(Value::as_str).map(str::to_string))
            .collect::<Vec<_>>();
        for expected in [
            "gmail_search_threads",
            "gmail_send_email",
            "gdocs_create_from_markdown",
            "gdrive_upload_file",
            "gcalendar_create_event",
            "gtasks_create",
            "gkeep_list_notes",
        ] {
            assert!(names.contains(&expected.to_string()), "missing {expected}");
        }
    }
}
