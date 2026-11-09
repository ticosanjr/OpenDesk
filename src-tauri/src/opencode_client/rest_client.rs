use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

/// OpenCode v2 responses generally wrap collections in a `data` field.
#[derive(Debug, Deserialize)]
struct DataEnvelope<T> {
    data: T,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub version: String,
    pub pid: u64,
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelVariantDto {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDto {
    pub id: String,
    #[serde(rename = "modelID", default)]
    pub model_id: Option<String>,
    #[serde(rename = "providerID", default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub variants: Vec<ModelVariantDto>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub cost: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRefDto {
    #[serde(rename = "providerID")]
    pub provider_id: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

impl ModelRefDto {
    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        let (base, variant) = match value.split_once('#') {
            Some((base, variant)) => {
                let variant = variant.trim();
                (
                    base.trim(),
                    if variant.is_empty() {
                        None
                    } else {
                        Some(variant.to_string())
                    },
                )
            }
            None => (value, None),
        };
        let (provider, id) = base.split_once('/')?;
        let provider = provider.trim();
        let id = id.trim();
        if provider.is_empty() || id.is_empty() {
            return None;
        }
        Some(Self {
            provider_id: provider.to_string(),
            id: id.to_string(),
            variant,
        })
    }

    /// Payload accepted by OpenCode: `variant` is omitted when absent. The
    /// schema rejects `variant: null`, which used to break every model without
    /// a variant (e.g. `provider/model`).
    pub fn to_payload(&self) -> Option<Value> {
        let provider = self.provider_id.trim();
        let id = self.id.trim();
        if provider.is_empty() || id.is_empty() {
            return None;
        }
        let mut payload = serde_json::json!({
            "providerID": provider,
            "id": id,
        });
        if let Some(variant) = self
            .variant
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            payload["variant"] = Value::String(variant.to_string());
        }
        Some(payload)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDto {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub model: Option<ModelRefDto>,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub permissions: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDiff {
    pub file: String,
    pub patch: String,
    #[serde(default)]
    pub additions: i64,
    #[serde(default)]
    pub deletions: i64,
    pub status: String,
}

#[derive(Clone)]
pub struct RestClient {
    client: reqwest::Client,
    /// SSE must not have a 120s total timeout; only the connect phase should
    /// be bounded. A long build would otherwise be killed mid-stream.
    stream_client: reqwest::Client,
    base_url: String,
    password: String,
}

impl RestClient {
    pub fn new(port: u16, password: String) -> Self {
        Self::new_with_url(&format!("http://127.0.0.1:{port}"), password)
    }

    pub fn new_with_url(base_url: &str, password: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("reqwest client configuration is valid");
        let stream_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("reqwest stream client configuration is valid");

        Self {
            client,
            stream_client,
            base_url: base_url.trim_end_matches('/').to_string(),
            password,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        self.client
            .request(method, format!("{}{}", self.base_url, path))
            .basic_auth("opencode", Some(&self.password))
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        directory: Option<&str>,
    ) -> Result<T> {
        let mut request = self.request(reqwest::Method::GET, path);
        if let Some(directory) = directory {
            request = request.query(&[("location[directory]", directory)]);
        }

        let response = request.send().await.context("OpenCode request failed")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenCode returned {status}: {body}");
        }
        response
            .json::<T>()
            .await
            .context("could not decode OpenCode response")
    }

    pub async fn info(&self) -> Result<ServerInfo> {
        self.get_json("/api/info", None).await
    }

    pub async fn models(&self, directory: Option<&str>) -> Result<Vec<ModelDto>> {
        let envelope: DataEnvelope<Vec<ModelDto>> = self.get_json("/api/model", directory).await?;
        Ok(envelope.data)
    }

    pub async fn agents(&self, directory: Option<&str>) -> Result<Vec<AgentDto>> {
        let envelope: DataEnvelope<Vec<AgentDto>> = self.get_json("/api/agent", directory).await?;
        Ok(envelope.data)
    }

    pub async fn skills(&self, directory: Option<&str>) -> Result<Vec<Value>> {
        let envelope: DataEnvelope<Vec<Value>> = self.get_json("/api/skill", directory).await?;
        Ok(envelope.data)
    }

    pub async fn commands(&self, directory: Option<&str>) -> Result<Vec<Value>> {
        let envelope: DataEnvelope<Vec<Value>> = self.get_json("/api/command", directory).await?;
        Ok(envelope.data)
    }

    pub async fn mcp_servers(&self, directory: Option<&str>) -> Result<Vec<Value>> {
        let envelope: DataEnvelope<Vec<Value>> = self.get_json("/api/mcp", directory).await?;
        Ok(envelope.data)
    }

    pub async fn create_session(
        &self,
        directory: &str,
        title: &str,
        agent: &str,
        model: Option<ModelRefDto>,
    ) -> Result<String> {
        let mut body = serde_json::json!({
            "title": title,
            "agent": agent,
            "location": { "directory": directory },
        });
        if let Some(payload) = model.as_ref().and_then(ModelRefDto::to_payload) {
            body["model"] = payload;
        }

        let response = self
            .request(reqwest::Method::POST, "/api/session")
            .json(&body)
            .send()
            .await
            .context("could not create OpenCode session")?;
        let status = response.status();
        let value: Value = response
            .json()
            .await
            .context("could not decode OpenCode session response")?;
        if !status.is_success() {
            anyhow::bail!("OpenCode session creation failed ({status}): {value}");
        }
        value
            .pointer("/data/id")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("OpenCode session response had no id: {value}"))
    }

    pub async fn prompt(&self, session_id: &str, text: &str) -> Result<()> {
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/api/session/{session_id}/prompt"),
            )
            .json(&serde_json::json!({ "text": text }))
            .send()
            .await
            .context("could not send OpenCode prompt")?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenCode prompt failed ({status}): {body}");
        }
        Ok(())
    }

    pub async fn interrupt(&self, session_id: &str) -> Result<()> {
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/api/session/{session_id}/interrupt"),
            )
            .send()
            .await
            .context("could not interrupt OpenCode session")?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenCode interrupt failed ({status}): {body}");
        }
        Ok(())
    }

    pub async fn wait_session(&self, session_id: &str) -> Result<()> {
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/api/experimental/session/{session_id}/wait"),
            )
            .send()
            .await
            .context("could not wait for OpenCode session")?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenCode wait failed ({status}): {body}");
        }
        Ok(())
    }

    pub async fn messages(&self, session_id: &str) -> Result<Value> {
        self.get_json_value(&format!("/api/session/{session_id}/message"))
            .await
    }

    pub async fn diff(&self, session_id: &str) -> Result<Vec<FileDiff>> {
        let envelope: DataEnvelope<Vec<FileDiff>> = self
            .get_json(&format!("/api/session/{session_id}/diff"), None)
            .await?;
        Ok(envelope.data)
    }

    pub async fn reply_permission(
        &self,
        session_id: &str,
        request_id: &str,
        decision: &str,
    ) -> Result<()> {
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/api/session/{session_id}/permission/{request_id}/reply"),
            )
            .json(&serde_json::json!({ "decision": decision }))
            .send()
            .await
            .context("could not reply to OpenCode permission request")?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenCode permission reply failed ({status}): {body}");
        }
        Ok(())
    }

    /// Ask the engine to compact the session context at the next step
    /// boundary. This is the engine-side durable compaction, not a prompt.
    pub async fn compact_session(&self, session_id: &str) -> Result<Value> {
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/api/session/{session_id}/compact"),
            )
            .json(&serde_json::json!({}))
            .send()
            .await
            .context("could not request session compaction")?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenCode compaction failed ({status}): {body}");
        }
        response.json().await.context("invalid compaction response")
    }

    /// Active context messages (everything after the last compaction).
    pub async fn context(&self, session_id: &str) -> Result<Vec<Value>> {
        let envelope: DataEnvelope<Vec<Value>> = self
            .get_json(&format!("/api/session/{session_id}/context"), None)
            .await?;
        Ok(envelope.data)
    }

    pub async fn event_response(&self) -> Result<reqwest::Response> {
        let response = self
            .stream_client
            .get(format!("{}/api/event", self.base_url))
            .basic_auth("opencode", Some(&self.password))
            .header("Accept", "text/event-stream")
            .header("Accept-Encoding", "identity")
            .send()
            .await
            .context("could not subscribe to OpenCode events")?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("OpenCode event stream failed ({status}): {body}");
        }
        Ok(response)
    }

    async fn get_json_value(&self, path: &str) -> Result<Value> {
        let response = self
            .request(reqwest::Method::GET, path)
            .send()
            .await
            .context("OpenCode request failed")?;
        let status = response.status();
        let value: Value = response
            .json()
            .await
            .context("could not decode OpenCode response")?;
        if !status.is_success() {
            anyhow::bail!("OpenCode returned {status}: {value}");
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Json, Router,
        http::{HeaderMap, StatusCode},
        response::IntoResponse,
        routing::{get, post},
    };

    fn authorized(headers: &HeaderMap) -> bool {
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .map(|value| value == "Basic b3BlbmNvZGU6c2VjcmV0")
            .unwrap_or(false)
    }

    async fn mock_server() -> (String, tokio::task::JoinHandle<()>) {
        let app = Router::new()
            .route(
                "/api/info",
                get(|headers: HeaderMap| async move {
                    if !authorized(&headers) {
                        return StatusCode::UNAUTHORIZED.into_response();
                    }
                    Json(serde_json::json!({ "version": "2.2.5", "pid": 42 })).into_response()
                }),
            )
            .route(
                "/api/model",
                get(|headers: HeaderMap| async move {
                    if !authorized(&headers) {
                        return StatusCode::UNAUTHORIZED.into_response();
                    }
                    Json(serde_json::json!({
                        "data": [{
                            "id": "glm",
                            "modelID": "glm-5.3",
                            "providerID": "opencode-go",
                            "cost": [{ "input": 1.0, "output": 2.0 }]
                        }]
                    }))
                    .into_response()
                }),
            )
            .route(
                "/api/skill",
                get(|| async {
                    Json(serde_json::json!({
                        "data": [{ "id": "opencode", "name": "OpenCode", "description": "Docs" }]
                    }))
                }),
            )
            .route(
                "/api/command",
                get(|| async {
                    Json(serde_json::json!({
                        "data": [{ "name": "init", "description": "guided setup" }]
                    }))
                }),
            )
            .route(
                "/api/mcp",
                get(|| async {
                    Json(serde_json::json!({
                        "data": [{ "name": "rea", "status": { "status": "connected" } }]
                    }))
                }),
            )
            .route(
                "/api/agent",
                get(|headers: HeaderMap| async move {
                    if !authorized(&headers) {
                        return StatusCode::UNAUTHORIZED.into_response();
                    }
                    Json(serde_json::json!({ "data": [{ "id": "build", "permissions": {} }] }))
                        .into_response()
                }),
            )
            .route(
                "/api/session",
                post(|headers: HeaderMap, Json(_body): Json<Value>| async move {
                    if !authorized(&headers) {
                        return StatusCode::UNAUTHORIZED.into_response();
                    }
                    Json(serde_json::json!({ "data": { "id": "ses-1" } })).into_response()
                }),
            )
            .route(
                "/api/session/{id}/prompt",
                post(|| async { StatusCode::NO_CONTENT }),
            )
            .route(
                "/api/session/{id}/interrupt",
                post(|| async { StatusCode::NO_CONTENT }),
            )
            .route(
                "/api/experimental/session/{id}/wait",
                post(|| async { StatusCode::NO_CONTENT }),
            )
            .route(
                "/api/session/{id}/message",
                get(|| async { Json(serde_json::json!({ "data": [] })) }),
            )
            .route(
                "/api/session/{id}/diff",
                get(|| async {
                    Json(serde_json::json!({
                        "data": [{ "file": "a.md", "patch": "+x", "status": "added" }]
                    }))
                }),
            )
            .route(
                "/api/session/{id}/permission/{request}/reply",
                post(|| async { StatusCode::NO_CONTENT }),
            )
            .route(
                "/api/session/{id}/compact",
                post(|| async { Json(serde_json::json!({ "data": { "id": "cmp-1" } })) }),
            )
            .route(
                "/api/session/{id}/context",
                get(|| async { Json(serde_json::json!({ "data": [] })) }),
            )
            .route(
                "/api/event",
                get(|| async { ([("content-type", "text/event-stream")], "data: ok\n\n") }),
            );

        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        (format!("http://{address}"), handle)
    }

    #[tokio::test]
    async fn exercises_the_v2_rest_surface_with_basic_auth() {
        let (url, handle) = mock_server().await;
        let client = RestClient::new_with_url(&url, "secret".into());
        assert_eq!(
            client.base_url().trim_end_matches('/'),
            url.trim_end_matches('/')
        );

        let info = client.info().await.expect("info");
        assert_eq!(info.version, "2.2.5");
        assert_eq!(info.pid, 42);

        let models = client.models(Some("/tmp/ws")).await.expect("models");
        assert_eq!(models[0].model_id.as_deref(), Some("glm-5.3"));
        let agents = client.agents(None).await.expect("agents");
        assert_eq!(agents[0].id, "build");
        let skills = client.skills(None).await.expect("skills");
        assert_eq!(skills[0]["id"], "opencode");
        let commands = client.commands(None).await.expect("commands");
        assert_eq!(commands[0]["name"], "init");
        let mcps = client.mcp_servers(None).await.expect("mcps");
        assert_eq!(mcps[0]["name"], "rea");

        let session = client
            .create_session("/tmp/ws", "Test", "build", None)
            .await
            .expect("session");
        assert_eq!(session, "ses-1");
        client.prompt(&session, "oi").await.expect("prompt");
        client.interrupt(&session).await.expect("interrupt");
        client.wait_session(&session).await.expect("wait");

        let messages = client.messages(&session).await.expect("messages");
        assert!(messages.get("data").is_some());
        let diff = client.diff(&session).await.expect("diff");
        assert_eq!(diff[0].file, "a.md");
        client
            .reply_permission(&session, "req-1", "once")
            .await
            .expect("permission");
        let compacted = client.compact_session(&session).await.expect("compact");
        assert_eq!(
            compacted.pointer("/data/id").and_then(Value::as_str),
            Some("cmp-1")
        );
        assert!(client.context(&session).await.expect("context").is_empty());
        let event = client.event_response().await.expect("event");
        assert!(event.status().is_success());

        handle.abort();
    }

    #[test]
    fn parses_and_serializes_model_refs_without_null_variant() {
        let plain = ModelRefDto::parse("opencode-go/glm-5.3").expect("plain");
        assert_eq!(plain.provider_id, "opencode-go");
        assert_eq!(plain.id, "glm-5.3");
        assert_eq!(plain.variant, None);
        assert_eq!(
            plain.to_payload().expect("payload"),
            serde_json::json!({"providerID": "opencode-go", "id": "glm-5.3"})
        );

        let variant = ModelRefDto::parse(" opencode-go/glm-5.3#high ").expect("variant");
        assert_eq!(variant.variant.as_deref(), Some("high"));
        assert_eq!(
            variant.to_payload().expect("payload"),
            serde_json::json!({"providerID": "opencode-go", "id": "glm-5.3", "variant": "high"})
        );

        let empty_variant = ModelRefDto::parse("opencode-go/glm-5.3#  ").expect("empty variant");
        assert_eq!(empty_variant.variant, None);
        assert_eq!(
            empty_variant.to_payload().expect("payload"),
            serde_json::json!({"providerID": "opencode-go", "id": "glm-5.3"})
        );
        assert!(ModelRefDto::parse("glm-5.3").is_none());
        assert!(ModelRefDto::parse("opencode-go/").is_none());
        assert!(ModelRefDto::parse("/glm-5.3").is_none());
        assert!(ModelRefDto::parse("  ").is_none());
    }

    #[tokio::test]
    async fn session_payload_never_contains_null_model_or_variant() {
        use std::sync::{Arc, Mutex};

        let bodies = Arc::new(Mutex::new(Vec::<Value>::new()));
        let capture = bodies.clone();
        let app = Router::new().route(
            "/api/session",
            post(move |Json(body): Json<Value>| {
                let capture = capture.clone();
                async move {
                    capture.lock().expect("bodies").push(body);
                    Json(serde_json::json!({ "data": { "id": "ses-1" } }))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .expect("listener");
        let address = listener.local_addr().expect("address");
        let handle = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        let client = RestClient::new_with_url(&format!("http://{address}"), "secret".into());

        client
            .create_session(
                "/tmp/ws",
                "Test",
                "build",
                Some(ModelRefDto::parse("provider/model").expect("model")),
            )
            .await
            .expect("plain model");
        client
            .create_session("/tmp/ws", "Test", "build", None)
            .await
            .expect("no model");

        let bodies = bodies.lock().expect("bodies");
        assert_eq!(
            bodies[0]["model"],
            serde_json::json!({ "providerID": "provider", "id": "model" })
        );
        assert!(bodies[0]["model"].get("variant").is_none());
        assert!(bodies[1].get("model").is_none());

        handle.abort();
    }

    #[tokio::test]
    async fn reports_transport_errors() {
        let client = RestClient::new_with_url("http://127.0.0.1:1", "secret".into());
        assert!(client.info().await.is_err());
    }

    #[tokio::test]
    async fn contract_rejects_unauthorized_requests() {
        let (url, handle) = mock_server().await;
        let client = RestClient::new_with_url(&url, "wrong".into());
        let error = client.info().await.expect_err("unauthorized");
        assert!(error.to_string().contains("401"), "{error}");
        handle.abort();
    }
}
