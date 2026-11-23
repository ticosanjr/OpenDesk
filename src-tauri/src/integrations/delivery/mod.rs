mod channel;
mod email;
mod google;
mod render;
mod run;
mod signal_channel;
mod sinks;
mod telegram_channel;
#[cfg(test)]
mod tests;
mod watchers;

use crate::{
    app_state::AppState,
    db::{artifacts, deliveries, runs, schedules, settings, task_watchers, tasks, workspaces},
    error::{AppError, AppResult},
    integrations::{
        google_api::GoogleApi,
        google_oauth::GoogleOAuth,
        signal,
        smtp_mailer::{self, EmailAttachment},
        telegram,
    },
    media::mime_for_path,
    paths::AppPaths,
    secrets::SecretStore,
    text::split_recipients,
};
use channel::DeliveryChannel;
use chrono::{Timelike, Utc};
use email::{deliver_email, retry_email};
use google::{export_google, retry_drive, retry_gdocs};
use html_escape::encode_text;
use render::{attachments_from_paths, build_attachments, build_html};
use sha2::{Digest, Sha256};
use signal_channel::deliver_signal;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use telegram_channel::deliver_telegram;

pub use run::deliver_run;
#[cfg(test)]
pub(crate) use sinks::retry_delivery;
pub(crate) use sinks::{retry_delay, with_retry};
pub use sinks::{retry_loop, retry_one};
#[cfg(test)]
pub(crate) use watchers::{hex_digest, novelty_hash};
