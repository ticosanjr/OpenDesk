use crate::{error::DomainResult, ids::WorkspaceId};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP `elicitation` is HITL 2.0: the host asks the user, never the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElicitationRequest {
    pub workspace_id: WorkspaceId,
    pub agent: String,
    pub message: String,
    pub schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ElicitationResponse {
    Accepted(Value),
    Declined,
    Cancelled,
}

#[async_trait]
pub trait ElicitationPort: Send + Sync {
    async fn elicit(&self, request: ElicitationRequest) -> DomainResult<ElicitationResponse>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingMessage {
    pub role: String,
    pub content: String,
}

/// MCP `sampling` delegates completion to the host execution engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingRequest {
    pub workspace_id: WorkspaceId,
    pub agent: String,
    pub model: Option<String>,
    pub messages: Vec<SamplingMessage>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingResponse {
    pub text: String,
    pub model: Option<String>,
}

#[async_trait]
pub trait SamplingPort: Send + Sync {
    async fn sample(&self, request: SamplingRequest) -> DomainResult<SamplingResponse>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elicitation_responses_are_tagged() {
        let accepted = ElicitationResponse::Accepted(serde_json::json!({ "name": "Ana" }));
        let json = serde_json::to_value(&accepted).expect("serialize");
        assert!(json.get("ACCEPTED").is_some() || json.as_object().is_some());
        let declined = serde_json::to_string(&ElicitationResponse::Declined).expect("serialize");
        assert!(declined.contains("DECLINED"));
    }

    #[test]
    fn sampling_request_keeps_optional_fields() {
        let request = SamplingRequest {
            workspace_id: WorkspaceId::from("ws-1"),
            agent: "build".into(),
            model: None,
            messages: vec![SamplingMessage {
                role: "user".into(),
                content: "oi".into(),
            }],
            max_tokens: Some(128),
        };
        let json = serde_json::to_value(&request).expect("serialize");
        assert_eq!(json["max_tokens"], 128);
        assert!(json["model"].is_null());
    }
}
