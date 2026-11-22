mod models;

use models::DataEnvelope;
pub use models::{AgentDto, FileDiff, ModelDto, ModelRefDto, ModelVariantDto, ServerInfo};

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

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
            .unwrap_or_else(|_| reqwest::Client::new());
        let stream_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

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

    /// Stateless one-shot generation. Unlike a session prompt this does not
    /// load agent instructions, tools or history, so it is the cheapest way to
    /// validate provider connectivity and quota.
    pub async fn generate_text(&self, text: &str, model: Option<ModelRefDto>) -> Result<String> {
        let mut body = serde_json::json!({ "prompt": text });
        if let Some(payload) = model.as_ref().and_then(ModelRefDto::to_payload) {
            body["model"] = payload;
        }
        let response = self
            .request(reqwest::Method::POST, "/api/experimental/generate/text")
            .json(&body)
            .send()
            .await
            .context("could not generate OpenCode text")?;
        let status = response.status();
        let raw = response.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("OpenCode generate failed ({status}): {raw}");
        }
        let value: Value =
            serde_json::from_str(&raw).context("could not decode OpenCode generate response")?;
        value
            .pointer("/data/text")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("OpenCode generate response had no text: {value}"))
    }

    pub async fn prompt(&self, session_id: &str, text: &str) -> Result<Value> {
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
        let value = response.json::<Value>().await.unwrap_or(Value::Null);
        if !status.is_success() {
            anyhow::bail!("OpenCode prompt failed ({status}): {value}");
        }
        Ok(value)
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

    /// Diff for one user message. Without the message id OpenCode returns an
    /// empty list, so chat turns must pass the id returned by the prompt.
    pub async fn diff_for_message(
        &self,
        session_id: &str,
        message_id: &str,
    ) -> Result<Vec<FileDiff>> {
        let envelope: DataEnvelope<Vec<FileDiff>> = self
            .get_json(
                &format!("/api/session/{session_id}/diff?messageID={message_id}"),
                None,
            )
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
mod tests;
