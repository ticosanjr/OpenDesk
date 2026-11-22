//! Unified conversation stream.
//!
//! Chat threads, task comments, HITL requests and chat turns share
//! `conversations` + `messages`. The public API keeps the V1 shapes so the
//! shell can switch storage without rewriting call sites.

pub mod interactions;
pub mod messages;
pub mod task;
pub mod threads;
pub mod turns;

pub use interactions::{CreateInteractionInput, TaskInteraction};
pub use messages::{CreateTaskMessageInput, TaskMessage};
pub use task::TaskConversation;
pub use threads::{ChatMessage, ChatThread, CreateThreadInput};
pub use turns::{ChatTurn, ChatTurnUsage, CreateChatTurnInput};
