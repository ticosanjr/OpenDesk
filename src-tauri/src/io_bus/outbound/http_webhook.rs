//! Outbound HTTP webhook sink with template rendering, Vault injection,
//! idempotency key and SSRF protection.

use crate::{
    db::io_bus::OutboundSink,
    error::{AppError, AppResult},
    io_bus::{IoContext, RunOutput, config_value, outbound::artifact_paths, ssrf::validate_target},
};
use reqwest::{Method, StatusCode, redirect::Policy};
use serde::Deserialize;
use std::{collections::BTreeMap, time::Duration};

#[derive(Debug, Deserialize)]
struct Config {
    url: String,
    #[serde(default = "default_method")]
    method: String,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    #[serde(default)]
    body_template: Option<String>,
    #[serde(default)]
    vault_ref: Option<String>,
    #[serde(default = "default_timeout")]
    timeout_secs: u64,
    #[serde(default)]
    expected_statuses: Vec<u16>,
    #[serde(default)]
    allow_private_network: bool,
}

fn default_method() -> String {
    "POST".into()
}
fn default_timeout() -> u64 {
    15
}

pub async fn run(
    ctx: &IoContext,
    sink: &OutboundSink,
    output: &RunOutput,
) -> AppResult<Vec<String>> {
    let config: Config = config_value(sink)?;
    let method = Method::from_bytes(config.method.to_uppercase().as_bytes())
        .map_err(|_| AppError::BadRequest(format!("método HTTP inválido: {}", config.method)))?;
    let url = validate_target(&config.url, config.allow_private_network).await?;
    let domain = url.host_str().unwrap_or_default().to_string();
    let body = config
        .body_template
        .as_deref()
        .map(|template| render_template(template, output))
        .unwrap_or_else(|| default_body(output));
    let body = resolve_vault_templates(ctx, sink, &body, &domain).await?;
    let expected = if config.expected_statuses.is_empty() {
        vec![200, 201, 202, 204]
    } else {
        config.expected_statuses.clone()
    };
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(config.timeout_secs.clamp(1, 30)))
        .redirect(Policy::none())
        .build()
        .map_err(|error| AppError::Internal(format!("http webhook client: {error}")))?;

    let mut last_error = String::new();
    for attempt in 0..3 {
        let mut request = client
            .request(method.clone(), url.clone())
            .header(
                "x-opendesk-idempotency-key",
                format!("{}:{}", output.run_id, sink.id),
            )
            .header("content-type", "application/json")
            .body(body.clone());
        for (name, value) in &config.headers {
            let value = render_template(value, output);
            let value = resolve_vault_templates(ctx, sink, &value, &domain).await?;
            request = request.header(name, value);
        }
        if let Some(reference) = config
            .vault_ref
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let secret = crate::db::vault::resolve_for_edge(
                &ctx.pool,
                reference,
                "sink",
                &sink.id,
                Some(&output.run_id),
                Some(&domain),
                false,
            )
            .await?;
            request = request.bearer_auth(secret);
        }
        let response = request
            .send()
            .await
            .map_err(|error| AppError::Internal(format!("http webhook: {error}")))?;
        let status = response.status();
        if expected.contains(&status.as_u16()) {
            return Ok(vec![format!("{} {} -> {}", method, url, status.as_u16())]);
        }
        last_error = format!("resposta {} de {}", status.as_u16(), url);
        if !(status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS) || attempt == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(200 * 2_u64.pow(attempt))).await;
    }
    Err(AppError::Internal(format!(
        "http webhook falhou: {last_error}"
    )))
}

fn render_template(template: &str, output: &RunOutput) -> String {
    let artifacts = artifact_paths(output)
        .iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("\n");
    template
        .replace("{{run.id}}", &output.run_id)
        .replace("{{run.workspace_id}}", &output.workspace_id)
        .replace("{{run.title}}", &output.title)
        .replace("{{run.summary}}", &output.summary)
        .replace(
            "{{run.output_directory}}",
            output
                .output_directory
                .as_deref()
                .unwrap_or(&output.workspace_root),
        )
        .replace("{{artifact.paths}}", &artifacts)
}

fn default_body(output: &RunOutput) -> String {
    serde_json::json!({
        "run_id": output.run_id,
        "workspace_id": output.workspace_id,
        "title": output.title,
        "summary": output.summary,
        "output_directory": output.output_directory,
        "artifacts": output.artifacts.iter().map(|artifact| artifact.file_path.clone()).collect::<Vec<_>>(),
    })
    .to_string()
}

async fn resolve_vault_templates(
    ctx: &IoContext,
    sink: &OutboundSink,
    text: &str,
    domain: &str,
) -> AppResult<String> {
    let mut output = text.to_string();
    while let Some(start) = output.find("{{vault:") {
        let Some(relative_end) = output[start..].find("}}") else {
            return Err(AppError::BadRequest(
                "placeholder {{vault:...}} não fechado".into(),
            ));
        };
        let end = start + relative_end;
        let entry = output[start + 8..end].trim().to_string();
        let secret = crate::db::vault::resolve_for_edge(
            &ctx.pool,
            &entry,
            "sink",
            &sink.id,
            None,
            Some(domain),
            false,
        )
        .await?;
        output.replace_range(start..end + 2, &secret);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rejects_private_targets_by_default() {
        assert!(
            validate_target("http://127.0.0.1:8080/hook", false)
                .await
                .is_err()
        );
        assert!(
            validate_target("http://169.254.169.254/latest/meta-data", false)
                .await
                .is_err()
        );
        assert!(
            validate_target("http://127.0.0.1:8080/hook", true)
                .await
                .is_ok()
        );
    }

    #[test]
    fn renders_run_placeholders() {
        let output = RunOutput {
            workspace_id: "ws".into(),
            workspace_root: "/tmp/ws".into(),
            output_directory: Some("/tmp/out".into()),
            task_id: None,
            run_id: "run-1".into(),
            title: "T".into(),
            summary: "S".into(),
            artifacts: Vec::new(),
        };
        let rendered = render_template("{{run.id}} {{run.output_directory}}", &output);
        assert_eq!(rendered, "run-1 /tmp/out");
    }

    #[tokio::test]
    async fn posts_rendered_template_to_private_target_when_opted_in() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("addr");
        let (tx, rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accept");
            let mut buffer = vec![0u8; 4096];
            let read = socket.read(&mut buffer).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            let _ = tx.send(request);
            let _ = socket
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok")
                .await;
        });
        let ctx = crate::test_support::context().await;
        let io = IoContext {
            pool: ctx.state.db.clone(),
            paths: ctx.state.paths.clone(),
        };
        let sink = OutboundSink {
            id: "sink-1".into(),
            workspace_id: ctx.workspace_id.clone(),
            name: "Hook".into(),
            sink_type: "HTTP_WEBHOOK".into(),
            config_json: serde_json::json!({
                "url": format!("http://{address}/hook"),
                "method": "POST",
                "allow_private_network": true,
                "body_template": "{\"run\":\"{{run.id}}\",\"title\":\"{{run.title}}\"}"
            })
            .to_string(),
            is_enabled: true,
            last_run_at: None,
            last_status: None,
            last_error: None,
            created_at: "now".into(),
        };
        let output = RunOutput {
            workspace_id: ctx.workspace_id.clone(),
            workspace_root: ctx.workspace_root.to_string_lossy().to_string(),
            output_directory: None,
            task_id: None,
            run_id: "run-1".into(),
            title: "Titulo".into(),
            summary: "Resumo".into(),
            artifacts: Vec::new(),
        };
        let result = run(&io, &sink, &output).await.expect("webhook");
        assert!(result[0].contains("200"));
        let request = rx.await.expect("request");
        assert!(request.contains("idempotency-key"));
        assert!(request.contains("run-1"));
    }

    #[tokio::test]
    async fn retries_transient_server_errors() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let address = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            for index in 0..3 {
                let (mut socket, _) = listener.accept().await.expect("accept");
                let mut buffer = vec![0u8; 2048];
                let _ = socket.read(&mut buffer).await;
                let status = if index < 2 {
                    "500 Internal Server Error"
                } else {
                    "200 OK"
                };
                let response = format!(
                    "HTTP/1.1 {status}\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok"
                );
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });
        let ctx = crate::test_support::context().await;
        let io = IoContext {
            pool: ctx.state.db.clone(),
            paths: ctx.state.paths.clone(),
        };
        let sink = OutboundSink {
            id: "sink-retry".into(),
            workspace_id: ctx.workspace_id.clone(),
            name: "Hook".into(),
            sink_type: "HTTP_WEBHOOK".into(),
            config_json: serde_json::json!({
                "url": format!("http://{address}/hook"),
                "allow_private_network": true,
                "body_template": "{}"
            })
            .to_string(),
            is_enabled: true,
            last_run_at: None,
            last_status: None,
            last_error: None,
            created_at: "now".into(),
        };
        let output = RunOutput {
            workspace_id: ctx.workspace_id.clone(),
            workspace_root: ctx.workspace_root.to_string_lossy().to_string(),
            output_directory: None,
            task_id: None,
            run_id: "run-retry".into(),
            title: "T".into(),
            summary: "S".into(),
            artifacts: Vec::new(),
        };
        run(&io, &sink, &output).await.expect("retry succeeds");
    }

    #[tokio::test]
    async fn rejects_unclosed_vault_placeholders() {
        let ctx = crate::test_support::context().await;
        let io = IoContext {
            pool: ctx.state.db.clone(),
            paths: ctx.state.paths.clone(),
        };
        let sink = OutboundSink {
            id: "sink-vault".into(),
            workspace_id: ctx.workspace_id.clone(),
            name: "Hook".into(),
            sink_type: "HTTP_WEBHOOK".into(),
            config_json: "{}".into(),
            is_enabled: true,
            last_run_at: None,
            last_status: None,
            last_error: None,
            created_at: "now".into(),
        };
        assert!(
            resolve_vault_templates(&io, &sink, "{{vault:missing", "example.com")
                .await
                .is_err()
        );
    }
}
