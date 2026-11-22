use serde::{Deserialize, Serialize};
use serde_json::Value;

/// OpenCode v2 responses generally wrap collections in a `data` field.
#[derive(Debug, Deserialize)]
pub(super) struct DataEnvelope<T> {
    pub(super) data: T,
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
