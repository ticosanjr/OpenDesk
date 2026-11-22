use super::paths::StudioPaths;
use crate::error::{AppError, AppResult};
use opendesk_storage::events::{EventBus, OpenDeskWsEvent};

use notify_debouncer_full::{
    DebounceEventResult, Debouncer, RecommendedCache, new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode},
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

const EXPECTATION_TTL: Duration = Duration::from_secs(2);
const MAX_EXPECTATIONS: usize = 512;

#[derive(Clone)]
struct ExpectedWrite {
    sha256: Option<String>,
    expires_at: Instant,
}

static EXPECTED_WRITES: OnceLock<Mutex<HashMap<PathBuf, ExpectedWrite>>> = OnceLock::new();

fn expectations() -> &'static Mutex<HashMap<PathBuf, ExpectedWrite>> {
    EXPECTED_WRITES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register an internal write so the watcher does not echo it back to the UI.
pub fn expect_write(path: &Path, sha256: Option<String>) {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut map = expectations()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let now = Instant::now();
    map.retain(|_, value| value.expires_at > now);
    if map.len() >= MAX_EXPECTATIONS {
        map.clear();
    }
    map.insert(
        canonical,
        ExpectedWrite {
            sha256,
            expires_at: now + EXPECTATION_TTL,
        },
    );
}

pub fn expect_delete(path: &Path) {
    expect_write(path, None);
}

fn is_transient(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    (name.starts_with('.') && name.ends_with(".tmp"))
        || name.ends_with(".swp")
        || name.ends_with(".swx")
}

fn should_suppress(path: &Path) -> bool {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut map = expectations()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let now = Instant::now();
    map.retain(|_, value| value.expires_at > now);
    let Some(expected) = map.get(&canonical).cloned() else {
        return false;
    };
    match expected.sha256 {
        None => {
            // Delete/rename can emit create/remove/modify events; keep the
            // suppression until the short TTL expires.
            true
        }
        Some(hash) => match std::fs::read(&canonical) {
            Ok(bytes) => {
                let actual = crate::config_sync::file_ops::sha256_hex(&bytes);
                if actual == hash {
                    true
                } else {
                    // Someone else changed the file inside the suppression
                    // window; let this and future events through.
                    map.remove(&canonical);
                    false
                }
            }
            Err(_) => false,
        },
    }
}

pub struct ConfigWatcher {
    debouncer: Option<Debouncer<RecommendedWatcher, RecommendedCache>>,
    watched: Vec<PathBuf>,
}

impl Default for ConfigWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigWatcher {
    pub fn new() -> Self {
        Self {
            debouncer: None,
            watched: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn watched(&self) -> &[PathBuf] {
        &self.watched
    }

    pub fn watch(
        &mut self,
        paths: &StudioPaths,
        workspace_id: &str,
        events: EventBus,
    ) -> AppResult<()> {
        // Dropping the old debouncer stops the previous workspace watches.
        self.debouncer = None;
        self.watched.clear();

        let global_dir = paths.global_dir.clone();
        let workspace_root = paths.workspace_root.clone();
        let callback_global_dir = global_dir.clone();
        let callback_workspace_root = workspace_root.clone();
        let callback_workspace_id = workspace_id.to_string();
        let bus = events.clone();

        let mut debouncer = new_debouncer(
            Duration::from_millis(150),
            None,
            move |result: DebounceEventResult| match result {
                Ok(events) => {
                    let mut seen = HashSet::new();
                    for debounced in events {
                        for path in debounced.event.paths {
                            if !seen.insert(path.clone()) {
                                continue;
                            }
                            if path.is_dir() || is_transient(&path) || should_suppress(&path) {
                                continue;
                            }
                            let entity =
                                entity_for(&path, &callback_workspace_root, &callback_global_dir);
                            bus.publish(OpenDeskWsEvent::FsSyncTriggered {
                                workspace_id: callback_workspace_id.clone(),
                                entity,
                                path: path.to_string_lossy().to_string(),
                            });
                        }
                    }
                }
                Err(errors) => {
                    for error in errors {
                        tracing::warn!(error = %error, "OpenCode config watcher error");
                    }
                }
            },
        )
        .map_err(|error| AppError::Internal(format!("could not start config watcher: {error}")))?;

        std::fs::create_dir_all(&global_dir)?;
        watch_path(
            &mut debouncer,
            &mut self.watched,
            &global_dir,
            RecursiveMode::Recursive,
        );

        if let Some(root) = &workspace_root {
            watch_path(
                &mut debouncer,
                &mut self.watched,
                root,
                RecursiveMode::NonRecursive,
            );
            watch_path(
                &mut debouncer,
                &mut self.watched,
                &root.join(".opencode"),
                RecursiveMode::Recursive,
            );
            watch_path(
                &mut debouncer,
                &mut self.watched,
                &root.join("input"),
                RecursiveMode::Recursive,
            );
            watch_path(
                &mut debouncer,
                &mut self.watched,
                &root.join("output"),
                RecursiveMode::Recursive,
            );
        }

        self.debouncer = Some(debouncer);
        Ok(())
    }
}

fn watch_path(
    debouncer: &mut Debouncer<RecommendedWatcher, RecommendedCache>,
    watched: &mut Vec<PathBuf>,
    path: &Path,
    mode: RecursiveMode,
) {
    if !path.exists() {
        return;
    }
    match debouncer.watch(path, mode) {
        Ok(()) => watched.push(path.to_path_buf()),
        Err(error) => {
            tracing::warn!(path = %path.display(), error = %error, "could not watch path");
        }
    }
}

fn entity_for(path: &Path, workspace_root: &Option<PathBuf>, global_dir: &Path) -> String {
    let value = path.to_string_lossy();
    if value.contains("/agents/") {
        return "agent".into();
    }
    if value.contains("/skills/") {
        return "skill".into();
    }
    if value.contains("/commands/") || value.contains("/command/") {
        return "command".into();
    }
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name == "opencode.jsonc" || name == "opencode.json")
        .unwrap_or(false)
    {
        return "config".into();
    }
    if workspace_root
        .as_ref()
        .map(|root| path.starts_with(root.join("output")))
        .unwrap_or(false)
    {
        return "output_file".into();
    }
    if path.starts_with(global_dir) {
        return "config".into();
    }
    "file".into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::opendesk_storage::events::OpenDeskWsEvent;
    use tempfile::TempDir;
    use tokio::time::{Duration as TokioDuration, timeout};

    #[tokio::test]
    async fn emits_fs_sync_event_for_external_edit() {
        let temp = TempDir::new().expect("temp");
        let workspace = temp.path().join("ws");
        let global = temp.path().join("global");
        std::fs::create_dir_all(workspace.join(".opencode/agents")).expect("agents");
        std::fs::create_dir_all(workspace.join("input")).expect("input");
        std::fs::create_dir_all(workspace.join("output")).expect("output");
        std::fs::create_dir_all(&global).expect("global");

        let paths = StudioPaths {
            global_dir: global,
            workspace_root: Some(workspace.clone()),
        };
        let bus = EventBus::new(64);
        let mut receiver = bus.subscribe();
        let mut watcher = ConfigWatcher::new();
        watcher
            .watch(&paths, "ws-test", bus.clone())
            .expect("watch");

        tokio::time::sleep(TokioDuration::from_millis(250)).await;
        std::fs::write(
            workspace.join(".opencode/agents/external.md"),
            "---\ndescription: external\n---\nbody\n",
        )
        .expect("write external file");

        let event = timeout(TokioDuration::from_secs(5), async {
            loop {
                let envelope = receiver.recv().await.expect("event");
                if let OpenDeskWsEvent::FsSyncTriggered {
                    entity,
                    workspace_id,
                    path,
                } = envelope.event
                    && entity == "agent"
                    && workspace_id == "ws-test"
                {
                    return path;
                }
            }
        })
        .await
        .expect("watcher timeout");

        assert!(event.ends_with("external.md"));
    }

    #[tokio::test]
    async fn internal_write_does_not_echo_but_external_does() {
        let temp = TempDir::new().expect("temp");
        let workspace = temp.path().join("ws");
        let global = temp.path().join("global");
        std::fs::create_dir_all(workspace.join(".opencode/agents")).expect("agents");
        std::fs::create_dir_all(workspace.join("input")).expect("input");
        std::fs::create_dir_all(workspace.join("output")).expect("output");
        std::fs::create_dir_all(&global).expect("global");
        let paths = StudioPaths {
            global_dir: global,
            workspace_root: Some(workspace.clone()),
        };
        let bus = EventBus::new(64);
        let mut receiver = bus.subscribe();
        let mut watcher = ConfigWatcher::new();
        watcher
            .watch(&paths, "ws-test", bus.clone())
            .expect("watch");
        tokio::time::sleep(TokioDuration::from_millis(250)).await;

        let agent = workspace.join(".opencode/agents/internal.md");
        crate::config_sync::file_ops::write_text_atomic(
            &agent,
            "---\ndescription: internal\n---\nbody\n",
            None,
            &temp.path().join("backups"),
        )
        .expect("internal write");
        match timeout(TokioDuration::from_millis(900), receiver.recv()).await {
            Err(_) => {}
            Ok(Ok(envelope)) => panic!("internal write emitted {envelope:?}"),
            Ok(Err(error)) => panic!("watcher error {error}"),
        }

        std::fs::write(&agent, "---\ndescription: external\n---\nbody\n").expect("external");
        let event = timeout(TokioDuration::from_secs(5), async {
            loop {
                let envelope = receiver.recv().await.expect("event");
                if let OpenDeskWsEvent::FsSyncTriggered { path, .. } = envelope.event {
                    return path;
                }
            }
        })
        .await
        .expect("external event");
        assert!(event.ends_with("internal.md"));
    }
}
