use crate::{app_state::AppState, db::io_bus, error::AppResult};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub struct IoBus {
    tasks: Mutex<HashMap<String, (CancellationToken, JoinHandle<()>)>>,
}

impl Default for IoBus {
    fn default() -> Self {
        Self::new()
    }
}

impl IoBus {
    pub fn new() -> Self {
        Self {
            tasks: Mutex::new(HashMap::new()),
        }
    }

    pub async fn start(self: &Arc<Self>, state: Arc<AppState>) -> AppResult<()> {
        self.stop().await;
        let sources = io_bus::all_enabled_inbound(&state.db).await?;
        for source in sources {
            let token = CancellationToken::new();
            let worker_token = token.clone();
            let worker_state = state.clone();
            let id = source.id.clone();
            let log_id = id.clone();
            let handle = tokio::spawn(async move {
                if let Err(error) =
                    crate::io_bus::inbound::run(worker_state, source, worker_token).await
                {
                    tracing::warn!(source = %log_id, error = %error, "io bus source stopped");
                }
            });
            self.tasks
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .insert(id, (token, handle));
        }
        Ok(())
    }

    pub async fn reload(self: &Arc<Self>, state: Arc<AppState>) -> AppResult<()> {
        self.start(state).await
    }

    pub async fn stop(&self) {
        let tasks = {
            let mut guard = self
                .tasks
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            std::mem::take(&mut *guard)
        };
        for (_, (token, handle)) in tasks {
            token.cancel();
            handle.abort();
        }
    }

    #[allow(dead_code)]
    pub fn running(&self) -> usize {
        self.tasks
            .lock()
            .map(|tasks| tasks.len())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_empty() {
        let bus = IoBus::new();
        assert_eq!(bus.running(), 0);
    }
}
