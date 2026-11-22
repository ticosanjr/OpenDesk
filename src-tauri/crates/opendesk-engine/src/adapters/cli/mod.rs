//! Generic CLI harness adapter.
//!
//! Claude Code, Codex, Pi and Hermes all expose a non-interactive mode that
//! prints one JSON object per line (or plain text). The adapter spawns the
//! configured command inside the workspace, normalizes stdout into
//! [`RuntimeEvent`]s and lets the queue settle the run exactly like OpenCode.

mod git_diff;
mod normalize;
mod process;

#[cfg(test)]
mod tests;

pub use git_diff::git_diffs;
pub use normalize::normalize_line;

use process::run_process;

use crate::{
    adapters::HarnessSpec,
    error::{AppError, AppResult},
    runtime::{
        AgentRuntimeAdapter, RuntimeCapabilities, RuntimeEvent, RuntimeEventStream, RuntimeRequest,
    },
};

use async_trait::async_trait;
use futures_util::stream;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

const CHANNEL_CAPACITY: usize = 4096;
const SUMMARY_LIMIT: usize = 4000;

struct Session {
    request: RuntimeRequest,
    sender: broadcast::Sender<RuntimeEvent>,
    receiver: Mutex<Option<broadcast::Receiver<RuntimeEvent>>>,
    cancel: Mutex<Option<CancellationToken>>,
}

pub struct CliAdapter {
    spec: HarnessSpec,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
}

impl CliAdapter {
    pub fn new(spec: HarnessSpec) -> AppResult<Self> {
        let Some(command) = spec.command.as_deref().map(str::trim) else {
            return Err(AppError::BadRequest(format!(
                "harness {} não tem comando configurado",
                spec.id
            )));
        };
        if command.is_empty() {
            return Err(AppError::BadRequest(format!(
                "harness {} não tem comando configurado",
                spec.id
            )));
        }
        Ok(Self {
            spec,
            sessions: Mutex::new(HashMap::new()),
        })
    }

    fn session(&self, id: &str) -> AppResult<Arc<Session>> {
        self.sessions
            .lock()
            .map_err(|_| AppError::Internal("harness sessions poisoned".into()))?
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::NotFound(format!("harness session {id}")))
    }
}

#[async_trait]
impl AgentRuntimeAdapter for CliAdapter {
    fn id(&self) -> &str {
        &self.spec.id
    }

    fn capabilities(&self) -> RuntimeCapabilities {
        self.spec.capabilities()
    }

    async fn start(&self, request: RuntimeRequest) -> AppResult<String> {
        let (sender, receiver) = broadcast::channel(CHANNEL_CAPACITY);
        let session_id = uuid::Uuid::now_v7().to_string();
        let session = Arc::new(Session {
            request,
            sender,
            receiver: Mutex::new(Some(receiver)),
            cancel: Mutex::new(None),
        });
        self.sessions
            .lock()
            .map_err(|_| AppError::Internal("harness sessions poisoned".into()))?
            .insert(session_id.clone(), session);
        Ok(session_id)
    }

    async fn prompt(&self, session_id: &str, prompt: &str) -> AppResult<()> {
        let session = self.session(session_id)?;
        let cancel = CancellationToken::new();
        *session
            .cancel
            .lock()
            .map_err(|_| AppError::Internal("harness cancel poisoned".into()))? =
            Some(cancel.clone());
        let spec = self.spec.clone();
        let request = session.request.clone();
        let sender = session.sender.clone();
        let prompt = prompt.to_string();
        tokio::spawn(async move {
            run_process(spec, request, prompt, sender, cancel).await;
        });
        Ok(())
    }

    async fn events(&self, session_id: &str) -> AppResult<RuntimeEventStream> {
        let session = self.session(session_id)?;
        let receiver = session
            .receiver
            .lock()
            .map_err(|_| AppError::Internal("harness events poisoned".into()))?
            .take()
            .ok_or_else(|| {
                AppError::BadRequest("eventos do harness só podem ser lidos uma vez".into())
            })?;
        Ok(boxed_stream(receiver))
    }

    async fn interrupt(&self, session_id: &str) -> AppResult<()> {
        let session = self.session(session_id)?;
        if let Some(cancel) = session
            .cancel
            .lock()
            .map_err(|_| AppError::Internal("harness cancel poisoned".into()))?
            .as_ref()
        {
            cancel.cancel();
        }
        Ok(())
    }
}

fn boxed_stream(receiver: broadcast::Receiver<RuntimeEvent>) -> RuntimeEventStream {
    struct State {
        receiver: broadcast::Receiver<RuntimeEvent>,
        done: bool,
    }
    let state = State {
        receiver,
        done: false,
    };
    Box::pin(stream::unfold(state, |mut state| async move {
        if state.done {
            return None;
        }
        loop {
            match state.receiver.recv().await {
                Ok(event) => {
                    // Errors may still be followed by the terminal event, so
                    // only RunFinished closes the stream.
                    if matches!(event, RuntimeEvent::RunFinished { .. }) {
                        state.done = true;
                    }
                    return Some((Ok(event), state));
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, harness = %"cli", "runtime events lagged");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }))
}
