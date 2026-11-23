use serde::Deserialize;

pub mod discord;
mod routes;
pub mod slack;
mod status;
mod webhook;
mod whatsapp;
pub mod whatsapp_qr;

pub use opendesk_channels::whatsapp::WhatsAppConfig;
pub use routes::*;
pub use status::*;
pub use webhook::*;
pub use whatsapp::*;

const MAX_TEXT: usize = 16_000;

#[derive(Debug, Deserialize, Default)]
pub struct WorkspaceQuery {
    pub workspace_id: Option<String>,
}
