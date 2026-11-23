//! Discord bot channel: config, gateway bridge, REST client and parsing.

mod bridge;
mod client;
mod config;
mod protocol;

pub use bridge::{DiscordBridge, DiscordEventHandler};
pub use client::{send_hitl, send_text, send_text_reply};
pub use config::{
    DiscordConfig, SECRET_NAMESPACE, SETTING, clear_token, load, save, set_token, token,
};
pub use protocol::{DiscordEvent, DiscordInteraction, IncomingDiscord, allowed, parse_dispatch};
