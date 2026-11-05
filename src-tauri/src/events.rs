use serde::{Deserialize, Serialize};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::broadcast;

/// Queue DTO used by the realtime protocol. The full queue implementation lands
/// in phase 3; keeping the contract here lets the frontend be written early.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueueJobDto {
    pub id: String,
    pub task_id: Option<String>,
    pub title: Option<String>,
    pub status: String,
    pub priority: i32,
    pub resolved_mode: String,
    pub resolved_agent: String,
    pub resolved_model: String,
    pub resolved_effort: String,
    pub enqueued_at: String,
    pub started_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArtifactProvenanceDto {
    pub id: String,
    pub workspace_id: String,
    pub task_id: Option<String>,
    pub run_id: String,
    pub file_path: String,
    pub absolute_path: String,
    pub file_type: String,
    pub action_type: String,
    pub created_by_agent: String,
    pub creation_reason: String,
    pub diff_patch: Option<String>,
    pub created_at: String,
}

/// The event union consumed by the React app over `/ws`.
///
/// The shape matches the specification: `event_type` + `payload`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", content = "payload")]
pub enum OpenDeskWsEvent {
    DaemonStatusChanged {
        online: bool,
        port: u16,
        active_jobs: usize,
        state: String,
        message: Option<String>,
        version: Option<String>,
    },
    FsSyncTriggered {
        workspace_id: String,
        entity: String,
        path: String,
    },
    QueueUpdated {
        workspace_id: String,
        jobs: Vec<QueueJobDto>,
    },
    RunStreamChunk {
        run_id: String,
        task_id: Option<String>,
        chunk_type: String,
        content: String,
    },
    ArtifactCreated {
        artifact: ArtifactProvenanceDto,
    },
    BudgetAlert {
        level: String,
        daily_spent: f64,
        daily_limit: f64,
        message: String,
    },
    /// Emitted when the frontend missed events (slow consumer or reconnect).
    /// The UI must refetch the relevant snapshots instead of trying to replay.
    ResyncRequired {
        reason: String,
    },
}

/// Adds a monotonic id so the frontend can detect gaps and resynchronise.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsEnvelope {
    pub event_id: u64,
    #[serde(flatten)]
    pub event: OpenDeskWsEvent,
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<WsEnvelope>,
    seq: Arc<AtomicU64>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            tx,
            seq: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn publish(&self, event: OpenDeskWsEvent) {
        let envelope = WsEnvelope {
            event_id: self.seq.fetch_add(1, Ordering::Relaxed),
            event,
        };
        // A missing receiver only means no UI is connected; the state remains in SQLite.
        let _ = self.tx.send(envelope);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WsEnvelope> {
        self.tx.subscribe()
    }

    #[allow(dead_code)]
    pub fn sender(&self) -> broadcast::Sender<WsEnvelope> {
        self.tx.clone()
    }
}
