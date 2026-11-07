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
    pub concurrency_group: Option<String>,
    pub lease_expires_at: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub resolved_mode: String,
    pub resolved_agent: String,
    pub resolved_model: String,
    pub resolved_effort: String,
    pub job_kind: String,
    pub output_dir: Option<String>,
    pub scheduled_for: Option<String>,
    pub enqueued_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub retry_count: i64,
    pub max_retries: i64,
    pub error_message: Option<String>,
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
        mode: String,
        binary_path: Option<String>,
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
    TaskUpdated {
        task_id: String,
        workspace_id: String,
        stage: String,
        phase_status: Option<String>,
    },
    PermissionRequested {
        request_id: String,
        run_id: String,
        task_id: Option<String>,
        workspace_id: String,
        action: String,
        resources: Vec<String>,
    },
    PermissionDecided {
        request_id: String,
        decision: String,
    },
    RecipeRunUpdated {
        run_id: String,
        recipe_id: String,
        workspace_id: String,
        status: String,
    },
    HitlRequested {
        token: String,
        run_id: String,
        workspace_id: String,
        checkpoint_id: String,
        interaction_type: String,
    },
    HitlResolved {
        token: String,
        status: String,
    },
    ProposalCreated {
        proposal_id: String,
        workspace_id: String,
        kind: String,
        title: String,
    },
    ProposalResolved {
        proposal_id: String,
        status: String,
        workspace_id: String,
    },
    RunFinished {
        run_id: String,
        task_id: Option<String>,
        status: String,
        estimated_cost_usd: f64,
        tokens_input: i64,
        tokens_output: i64,
    },
    ScheduleUpdated {
        schedule_id: String,
        workspace_id: String,
    },
    GoogleConnectionUpdated {
        workspace_id: String,
        connected: bool,
        email: Option<String>,
    },
    EngineUpdateProgress {
        phase: String,
        message: Option<String>,
        ok: Option<bool>,
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
