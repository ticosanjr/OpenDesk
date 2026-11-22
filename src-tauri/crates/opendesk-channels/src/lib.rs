//! OpenDesk channels: provider-agnostic registry and notification routing.
//!
//! The adapters themselves (Telegram long polling, WhatsApp Cloud API, e-mail,
//! Signal and Web Push) still live in the shell; extracting their I/O requires
//! the engine/storage ports that remain in `opendesk-core`. This crate owns the
//! policy that is already independent: feature-gated channel registry and the
//! `NotificationRouter`.

pub mod registry;
pub mod router;

pub use registry::{ChannelDescriptor, enabled_channels};
pub use router::{NotificationDecision, NotificationRouter, RouterConfig};
