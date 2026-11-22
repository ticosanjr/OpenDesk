//! Pure OpenDesk domain.
//!
//! This crate has no SQL, HTTP, filesystem or Tauri dependency. Storage,
//! engine, channels and the shell depend on it; it never depends on them.
//!
//! - `ids`: typed identifiers;
//! - `task`/`run`: persisted state machines;
//! - `hitl`: HITL 2.0 vocabulary;
//! - `conversation`: unified chat, comments, HITL and turns;
//! - `event`: state-change events and the sink port;
//! - `ports`: clock, notifier, dispatcher and repository contracts.

pub mod channel;
pub mod conversation;
pub mod error;
pub mod event;
pub mod hitl;
pub mod ids;
pub mod mcp;
pub mod notification;
pub mod ports;
pub mod run;
pub mod task;

pub use channel::{
    Attachment, ChannelAdapter, ChannelCapabilities, ChannelThread, DeliveryReceipt,
    NormalizedEnvelope,
};
pub use conversation::{
    Conversation, Message, MessageKind, MessageRole, MessageStatus, NewMessage,
};
pub use error::{DomainError, DomainResult};
pub use event::{DomainEvent, EventSink, NoopEventSink};
pub use hitl::{HitlKind, HitlOutcome, HitlStatus};
pub use ids::{ConversationId, JobId, MessageId, RunId, ScheduleId, TaskId, WorkspaceId};
pub use mcp::{
    ElicitationPort, ElicitationRequest, ElicitationResponse, SamplingMessage, SamplingPort,
    SamplingRequest, SamplingResponse,
};
pub use notification::{CareLevel, QuietHours};
pub use ports::{
    Clock, ConversationRepository, NotificationIntent, NotificationLevel, Notifier, SystemClock,
    TaskDispatcher, TaskStatusRepository,
};
pub use run::RunStatus;
pub use task::TaskStatus;
