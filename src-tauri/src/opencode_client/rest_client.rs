use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
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
}
