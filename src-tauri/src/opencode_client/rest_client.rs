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
    #[serde(default)]
    pub variant: Option<String>,
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
    base_url: String,
    password: String,
}

impl RestClient {
    pub fn new(port: u16, password: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .expect("reqwest client configuration is valid");

        Self {
            client,
            base_url: format!("http://127.0.0.1:{port}"),
            password,
        }
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
        Ok(response
            .json::<T>()
            .await
            .context("could not decode OpenCode response")?)
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

    pub async fn create_session(
        &self,
        directory: &str,
        title: &str,
        agent: &str,
        model: Option<ModelRefDto>,
    ) -> Result<String> {
        let body = serde_json::json!({
            "title": title,
            "agent": agent,
            "model": model,
            "location": { "directory": directory },
        });

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
        reply: &str,
    ) -> Result<()> {
        let response = self
            .request(
                reqwest::Method::POST,
                &format!("/api/session/{session_id}/permission/{request_id}/reply"),
            )
            .json(&serde_json::json!({ "reply": reply }))
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

    pub async fn event_response(&self) -> Result<reqwest::Response> {
        let response = self
            .request(reqwest::Method::GET, "/api/event")
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
