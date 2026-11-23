use serde::Deserialize;

mod routes;
mod status;
mod webhook;
mod whatsapp;

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
