//! MCP Streamable HTTP sessions.
//!
//! The HTTP gateway keeps a small per-session event buffer so a client can
//! reconnect with `Last-Event-ID` and resume without losing notifications. The
//! session is also the boundary for server-initiated requests (elicitation and
//! sampling) added in E4/E5.

use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, oneshot};

const BUFFER: usize = 128;
const IDLE: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone)]
pub struct McpEvent {
    pub id: u64,
    pub payload: Value,
}

pub struct McpSession {
    pub id: String,
    pub workspace_id: String,
    pub agent: String,
    pub phase: String,
    pub role: String,
    tx: broadcast::Sender<McpEvent>,
    buffer: Mutex<VecDeque<McpEvent>>,
    next_id: AtomicU64,
    pending: Mutex<HashMap<String, oneshot::Sender<Value>>>,
    last_seen: Mutex<Instant>,
}

impl McpSession {
    fn new(id: String, workspace_id: String, agent: String, phase: String, role: String) -> Self {
        let (tx, _) = broadcast::channel(BUFFER);
        Self {
            id,
            workspace_id,
            agent,
            phase,
            role,
            tx,
            buffer: Mutex::new(VecDeque::new()),
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            last_seen: Mutex::new(Instant::now()),
        }
    }

    pub fn publish(&self, payload: Value) -> u64 {
        self.touch();
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let event = McpEvent { id, payload };
        {
            let mut buffer = self.buffer.lock().expect("mcp buffer");
            buffer.push_back(event.clone());
            while buffer.len() > BUFFER {
                buffer.pop_front();
            }
        }
        let _ = self.tx.send(event);
        id
    }

    pub fn subscribe(
        &self,
        last_event_id: Option<u64>,
    ) -> (Vec<McpEvent>, broadcast::Receiver<McpEvent>) {
        self.touch();
        let replay = self
            .buffer
            .lock()
            .expect("mcp buffer")
            .iter()
            .filter(|event| last_event_id.is_none_or(|last| event.id > last))
            .cloned()
            .collect();
        (replay, self.tx.subscribe())
    }

    /// Sends a server-initiated JSON-RPC request over the client stream and
    /// waits for the matching response. Used by elicitation and sampling.
    pub async fn request(&self, method: &str, params: Value, timeout: Duration) -> Option<Value> {
        let id = format!("od-{}", uuid::Uuid::now_v7());
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .expect("mcp pending")
            .insert(id.clone(), tx);
        self.publish(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }));
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(value)) => Some(value),
            _ => {
                self.pending.lock().expect("mcp pending").remove(&id);
                None
            }
        }
    }

    pub fn resolve_client_response(&self, response: &Value) {
        let id = response.get("id").and_then(|value| {
            value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_u64().map(|number| number.to_string()))
        });
        let Some(id) = id else { return };
        if let Some(tx) = self.pending.lock().expect("mcp pending").remove(&id) {
            let _ = tx.send(response.clone());
        }
    }

    pub fn touch(&self) {
        *self.last_seen.lock().expect("mcp last seen") = Instant::now();
    }

    fn idle(&self) -> bool {
        self.last_seen.lock().expect("mcp last seen").elapsed() > IDLE
    }
}

#[derive(Default)]
pub struct McpSessionRegistry {
    sessions: Mutex<HashMap<String, Arc<McpSession>>>,
}

impl McpSessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(
        &self,
        workspace_id: &str,
        agent: &str,
        phase: &str,
        role: &str,
    ) -> Arc<McpSession> {
        self.sweep();
        let id = uuid::Uuid::now_v7().to_string();
        let session = Arc::new(McpSession::new(
            id.clone(),
            workspace_id.to_string(),
            agent.to_string(),
            phase.to_string(),
            role.to_string(),
        ));
        self.sessions
            .lock()
            .expect("mcp sessions")
            .insert(id, session.clone());
        session
    }

    pub fn get(&self, id: &str) -> Option<Arc<McpSession>> {
        let session = self.sessions.lock().expect("mcp sessions").get(id).cloned();
        if let Some(session) = &session {
            session.touch();
        }
        session
    }

    pub fn remove(&self, id: &str) -> bool {
        self.sessions
            .lock()
            .expect("mcp sessions")
            .remove(id)
            .is_some()
    }

    pub fn sweep(&self) {
        self.sessions
            .lock()
            .expect("mcp sessions")
            .retain(|_, session| !session.idle());
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.sessions.lock().expect("mcp sessions").len()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn buffers_events_for_resumption() {
        let registry = McpSessionRegistry::new();
        let session = registry.create("ws", "build", "ALL", "");
        session.publish(json!({ "n": 1 }));
        session.publish(json!({ "n": 2 }));
        session.publish(json!({ "n": 3 }));
        let (replay, _rx) = session.subscribe(Some(1));
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].payload["n"], 2);
        assert_eq!(replay[1].id, 3);
        let (all, _rx) = session.subscribe(None);
        assert_eq!(all.len(), 3);
    }

    #[tokio::test]
    async fn resolves_a_server_initiated_request() {
        let registry = McpSessionRegistry::new();
        let session = registry.create("ws", "build", "ALL", "");
        let responder = session.clone();
        tokio::spawn(async move {
            let (replay, mut rx) = responder.subscribe(None);
            let event = match replay.first().cloned() {
                Some(event) => event,
                None => rx.recv().await.expect("event"),
            };
            let id = event.payload["id"].clone();
            responder.resolve_client_response(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": { "action": "accept", "content": { "name": "ok" } }
            }));
        });
        let response = session
            .request(
                "elicitation/create",
                json!({ "message": "nome?" }),
                Duration::from_secs(2),
            )
            .await
            .expect("response");
        assert_eq!(response["result"]["content"]["name"], "ok");
    }

    #[test]
    fn removes_and_sweeps_sessions() {
        let registry = McpSessionRegistry::new();
        let session = registry.create("ws", "build", "ALL", "");
        assert_eq!(registry.len(), 1);
        assert!(registry.get(&session.id).is_some());
        assert!(registry.remove(&session.id));
        assert!(registry.get(&session.id).is_none());
        assert!(registry.is_empty());
    }
}
