use super::DropzoneConfig;
use crate::app_state::AppState;
use std::{path::PathBuf, sync::Arc, time::Duration};
use tokio_util::sync::CancellationToken;

pub(super) fn spawn(
    state: Arc<AppState>,
    config: DropzoneConfig,
    dropzone: PathBuf,
    cancel: CancellationToken,
    remote: String,
) {
    tokio::spawn(async move {
        let key_path = if let Some(key) = config.ssh_key_ref.as_deref() {
            crate::secrets::SecretStore::new(&state.paths)
                .get("io_bus", key)
                .ok()
                .flatten()
        } else {
            config.ssh_key_path.clone()
        };
        let rsync_config = crate::io_bus::outbound::rsync::RsyncConfig {
            ssh_key_path: key_path,
            port: config.port,
            delete: config.delete,
            checksum: config.checksum,
            timeout_secs: Some(120),
            extra_args: Vec::new(),
        };
        loop {
            if cancel.is_cancelled() {
                break;
            }
            let _ = crate::io_bus::outbound::rsync::pull(&remote, &dropzone, &rsync_config).await;
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = tokio::time::sleep(Duration::from_secs(config.interval_secs.unwrap_or(300).clamp(15, 3600))) => {}
            }
        }
    });
}
