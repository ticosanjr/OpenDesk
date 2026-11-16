//! Harness-agnostic execution contract.
//!
//! OpenCode V2 is the first adapter; Claude Code, Codex, Pi and Hermes plug into
//! the same normalized event stream so the queue, HITL, artifacts, budget and
//! observability do not depend on a specific tool.

use crate::error::{AppError, AppResult};
use async_trait::async_trait;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    pin::Pin,
    sync::{Arc, RwLock},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeCapabilities {
    pub streaming: bool,
    pub tools: bool,
    pub permissions: bool,
    pub reasoning: bool,
    pub usage: bool,
    pub diffs: bool,
    pub sessions: bool,
    pub sandbox: bool,
}

impl Default for RuntimeCapabilities {
    fn default() -> Self {
        Self {
            streaming: true,
            tools: true,
            permissions: true,
            reasoning: false,
            usage: true,
            diffs: true,
            sessions: true,
            sandbox: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeRequest {
    pub workspace_root: String,
    pub agent: String,
    pub model: Option<String>,
    pub prompt: String,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub sandbox: crate::sandbox::SandboxPolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeFileDiff {
    pub path: String,
    pub status: String,
    pub patch: String,
    pub additions: i64,
    pub deletions: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeEvent {
    RunStarted {
        run_id: String,
        session_id: Option<String>,
    },
    TextDelta {
        delta: String,
    },
    ReasoningDelta {
        delta: String,
    },
    ToolCall {
        call_id: String,
        name: String,
        input: Value,
        status: String,
    },
    PermissionAsked {
        request_id: String,
        action: String,
        resources: Vec<String>,
    },
    Usage {
        input_tokens: i64,
        output_tokens: i64,
        cache_read: i64,
        cache_write: i64,
        cost_usd: f64,
    },
    Diff {
        files: Vec<RuntimeFileDiff>,
    },
    Artifact {
        path: String,
        action: String,
    },
    RunFinished {
        status: String,
        summary: Option<String>,
    },
    Error {
        code: String,
        message: String,
    },
}

pub type RuntimeEventStream = Pin<Box<dyn Stream<Item = AppResult<RuntimeEvent>> + Send>>;

#[async_trait]
pub trait AgentRuntimeAdapter: Send + Sync {
    fn id(&self) -> &str;
    fn capabilities(&self) -> RuntimeCapabilities;

    /// Creates a session and returns its id.
    async fn start(&self, request: RuntimeRequest) -> AppResult<String>;

    /// Sends one turn to an existing session.
    async fn prompt(&self, session_id: &str, prompt: &str) -> AppResult<()>;

    /// Streams normalized events for the active turn/session.
    async fn events(&self, session_id: &str) -> AppResult<RuntimeEventStream>;

    async fn interrupt(&self, session_id: &str) -> AppResult<()>;
}

#[derive(Default)]
pub struct AdapterRegistry {
    adapters: RwLock<HashMap<String, Arc<dyn AgentRuntimeAdapter>>>,
}

impl AdapterRegistry {
    pub fn register(&self, adapter: Arc<dyn AgentRuntimeAdapter>) -> AppResult<()> {
        let id = adapter.id().to_string();
        if id.trim().is_empty() {
            return Err(AppError::BadRequest(
                "runtime adapter precisa de um id".into(),
            ));
        }
        self.adapters
            .write()
            .map_err(|_| AppError::Internal("runtime adapter registry poisoned".into()))?
            .insert(id, adapter);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<Arc<dyn AgentRuntimeAdapter>> {
        self.adapters
            .read()
            .ok()
            .and_then(|adapters| adapters.get(id).cloned())
    }

    pub fn list(&self) -> Vec<(String, RuntimeCapabilities)> {
        let mut list = self
            .adapters
            .read()
            .map(|adapters| {
                adapters
                    .iter()
                    .map(|(id, adapter)| (id.clone(), adapter.capabilities()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        list.sort_by(|left, right| left.0.cmp(&right.0));
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;

    struct DummyAdapter;

    #[async_trait]
    impl AgentRuntimeAdapter for DummyAdapter {
        fn id(&self) -> &str {
            "dummy"
        }

        fn capabilities(&self) -> RuntimeCapabilities {
            RuntimeCapabilities::default()
        }

        async fn start(&self, _request: RuntimeRequest) -> AppResult<String> {
            Ok("session-1".into())
        }

        async fn prompt(&self, _session_id: &str, _prompt: &str) -> AppResult<()> {
            Ok(())
        }

        async fn events(&self, _session_id: &str) -> AppResult<RuntimeEventStream> {
            Ok(Box::pin(stream::empty()))
        }

        async fn interrupt(&self, _session_id: &str) -> AppResult<()> {
            Ok(())
        }
    }

    #[test]
    fn normalizes_runtime_events_as_tagged_json() {
        let event = RuntimeEvent::ToolCall {
            call_id: "call-1".into(),
            name: "read".into(),
            input: serde_json::json!({ "path": "a.md" }),
            status: "completed".into(),
        };
        let value = serde_json::to_value(&event).expect("serialize");
        assert_eq!(value["type"], "tool_call");
        let decoded: RuntimeEvent = serde_json::from_value(value).expect("deserialize");
        assert!(matches!(decoded, RuntimeEvent::ToolCall { .. }));
    }

    #[test]
    fn registers_and_lists_adapters() {
        let registry = AdapterRegistry::default();
        registry.register(Arc::new(DummyAdapter)).expect("register");
        assert!(registry.get("dummy").is_some());
        assert_eq!(registry.list().len(), 1);
    }
}
