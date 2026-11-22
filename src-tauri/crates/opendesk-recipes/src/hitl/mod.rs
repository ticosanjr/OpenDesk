//! HITL checkpoints: request, wait, resolution and channel replies.

mod replies;
mod service;
mod types;

pub use replies::{extract_token, parse_reply, reply_instructions};
pub use service::{request, resolve, wait};
pub use types::{HitlOutcome, HitlRequestPayload, HitlResolution};

#[cfg(test)]
mod tests;
