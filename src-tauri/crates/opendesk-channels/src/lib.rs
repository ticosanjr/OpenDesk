//! OpenDesk channels: provider-agnostic registry and notification routing.
//!
//! The adapters themselves (Telegram long polling, WhatsApp Cloud API, e-mail,
//! Signal and Web Push) still live in the shell; extracting their I/O requires
//! the engine/storage ports that remain in `opendesk-core`. This crate owns the
//! policy that is already independent: feature-gated channel registry and the
//! `NotificationRouter`.

pub mod error;

use std::sync::OnceLock;

/// Shared HTTP client so Discord/Slack/WhatsApp requests reuse the connection
/// pool instead of building a new TLS stack per message.
pub(crate) fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

#[cfg(feature = "ch-discord")]
pub mod discord;
#[cfg(feature = "ch-signal")]
pub mod signal;
#[cfg(feature = "ch-slack")]
pub mod slack;
#[cfg(feature = "ch-telegram")]
pub mod telegram;
#[cfg(feature = "ch-whatsapp")]
pub mod whatsapp;

#[cfg(feature = "ch-email")]
pub mod email;
pub mod registry;
pub mod router;
pub mod transport;
#[cfg(feature = "ch-webpush")]
pub mod webpush;

pub use registry::{ChannelDescriptor, enabled_channels};
pub use router::{NotificationDecision, NotificationRouter, RouterConfig, TrustLevel};
pub use transport::ChannelTransport;
