//! Slack channel: Socket Mode bridge, Web API client and parsing.

mod bridge;
mod client;
mod config;
mod protocol;

pub use bridge::{SlackBridge, SlackEventHandler};
pub use client::{connections_open, send_hitl, send_text, send_text_in_thread};
pub use config::{
    SECRET_NAMESPACE, SETTING, SlackConfig, app_token, bot_token, clear_tokens, load, save,
    set_tokens,
};
pub use protocol::{IncomingSlack, SlackAction, SlackEvent, allowed, parse_envelope};
