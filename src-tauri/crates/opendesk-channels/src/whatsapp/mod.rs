//! WhatsApp Cloud API adapter: persisted config, Graph API sends, Meta webhook
//! HMAC verification and inbound payload parsing.
//!
//! The HTTP surface (`/api/channels/whatsapp`, `/api/webhooks/channels/whatsapp`)
//! and the generic channel registry stay in the Axum layer; this module owns
//! only the parts that are specific to WhatsApp and independent from the app
//! state. Settings live in `app_settings` and the token/app secret stay in the
//! encrypted `SecretStore`, so HTTP handlers can pass a `SqlitePool` and a
//! `SecretStore` instead of the whole shell state.

mod config;
#[cfg(feature = "ch-whatsapp-qr")]
mod qr;
mod send;
mod webhook;

pub use config::{
    WEBHOOK_HEADER, WEBHOOK_NAMESPACE, WHATSAPP_SECRET_NAMESPACE, WHATSAPP_SETTING, WhatsAppConfig,
    apply_credentials, connected, load_config, save_config, validate_config,
};
#[cfg(feature = "ch-whatsapp-qr")]
pub use qr::{
    QrEvent, WhatsAppQrBridge, WhatsAppQrConfig, WhatsAppQrEventHandler, allowed as qr_allowed,
    load as load_qr_config, parse_line as parse_qr_line, save as save_qr_config,
};
pub use send::{SentMessage, send_text};
pub use webhook::{MetaMessage, extract_meta_messages, media_extension, verify_signature};

#[cfg(test)]
mod tests;
