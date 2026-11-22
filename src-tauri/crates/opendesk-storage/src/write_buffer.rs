//! Serialized, coalescing write buffer for high-frequency lifecycle writes.
//!
//! Queue heartbeats and node presence updates happen every few seconds per
//! run/node. Letting every task write directly to SQLite creates a write storm
//! that competes with the API, the watcher and the queue worker. All of these
//! writes are idempotent "touch" updates, so they can be batched:
//!
//! * a single writer owns the batch transaction (`BEGIN IMMEDIATE`),
//! * repeated updates for the same job/node collapse into one statement,
//! * producers await the bounded channel, so a slow writer applies bounded
//!   backpressure instead of failing with `SQLITE_BUSY`.
//!
//! State transitions (`RUNNING`, `SUCCESS`, `OFFLINE`, …) keep their immediate
//! writes so callers observe them synchronously.

use sqlx::SqlitePool;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::{mpsc, oneshot};

const HEARTBEAT_SQL: &str =
    "UPDATE execution_queue SET last_heartbeat_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
        lease_expires_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '+60 seconds')
     WHERE id = ? AND status = 'RUNNING'";

const NODE_SEEN_SQL: &str =
    "UPDATE workers SET last_seen_at = strftime('%Y-%m-%dT%H:%M:%fZ','now'),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
     WHERE id = ?";

#[derive(Debug, Clone)]
pub enum BufferedWrite {
    JobHeartbeat { job_id: String },
    NodeSeen { node_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum WriteKey {
    Job(String),
    Node(String),
}

impl BufferedWrite {
    fn key(&self) -> WriteKey {
        match self {
            Self::JobHeartbeat { job_id } => WriteKey::Job(job_id.clone()),
            Self::NodeSeen { node_id } => WriteKey::Node(node_id.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct WriteBufferMetrics {
    pub queued: u64,
    pub coalesced: u64,
    pub flushed: u64,
    pub failed: u64,
    pub dropped: u64,
    pub pending: u64,
}

#[derive(Default)]
struct Counters {
    queued: AtomicU64,
    coalesced: AtomicU64,
    flushed: AtomicU64,
    failed: AtomicU64,
    dropped: AtomicU64,
    pending: AtomicU64,
}

enum Command {
    Write(BufferedWrite),
    Shutdown(oneshot::Sender<()>),
}

/// Handle to the single writer task.
pub struct WriteBuffer {
    sender: mpsc::Sender<Command>,
    counters: Arc<Counters>,
}

impl WriteBuffer {
    /// Starts the writer task. `capacity` bounds the number of pending writes
    /// before producers are backpressured; `interval` is the maximum batching
    /// delay (250 ms in production).
    pub fn start(pool: SqlitePool, interval: Duration, capacity: usize) -> Arc<Self> {
        let (sender, receiver) = mpsc::channel(capacity.max(1));
        let counters = Arc::new(Counters::default());
        let task_counters = counters.clone();
        tokio::spawn(async move {
            writer_loop(pool, interval, receiver, task_counters).await;
        });
        Arc::new(Self { sender, counters })
    }

    /// Enqueues a write and waits for capacity. Heartbeats are idempotent, so
    /// waiting is safer than dropping and leaving a lease to expire.
    pub async fn enqueue(&self, write: BufferedWrite) -> bool {
        match self.sender.send(Command::Write(write)).await {
            Ok(()) => {
                self.counters.queued.fetch_add(1, Ordering::Relaxed);
                true
            }
            Err(_) => {
                self.counters.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    pub async fn job_heartbeat(&self, job_id: &str) -> bool {
        self.enqueue(BufferedWrite::JobHeartbeat {
            job_id: job_id.to_string(),
        })
        .await
    }

    pub async fn node_seen(&self, node_id: &str) -> bool {
        self.enqueue(BufferedWrite::NodeSeen {
            node_id: node_id.to_string(),
        })
        .await
    }

    /// Flushes and stops the writer task. Waits until every write enqueued
    /// before this call has been committed.
    pub async fn shutdown(&self) {
        let (ack, wait) = oneshot::channel();
        if self.sender.send(Command::Shutdown(ack)).await.is_ok() {
            let _ = wait.await;
        }
    }

    pub fn metrics(&self) -> WriteBufferMetrics {
        WriteBufferMetrics {
            queued: self.counters.queued.load(Ordering::Relaxed),
            coalesced: self.counters.coalesced.load(Ordering::Relaxed),
            flushed: self.counters.flushed.load(Ordering::Relaxed),
            failed: self.counters.failed.load(Ordering::Relaxed),
            dropped: self.counters.dropped.load(Ordering::Relaxed),
            pending: self.counters.pending.load(Ordering::Relaxed),
        }
    }
}

impl Drop for WriteBuffer {
    fn drop(&mut self) {
        // Best effort: the process is exiting anyway and the sender closing
        // stops the task after flushing whatever is already queued.
        let (ack, _wait) = oneshot::channel();
        let _ = self.sender.try_send(Command::Shutdown(ack));
    }
}

async fn writer_loop(
    pool: SqlitePool,
    interval: Duration,
    mut receiver: mpsc::Receiver<Command>,
    counters: Arc<Counters>,
) {
    let mut pending: HashMap<WriteKey, BufferedWrite> = HashMap::new();
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            command = receiver.recv() => match command {
                Some(Command::Write(write)) => {
                    let key = write.key();
                    if pending.insert(key, write).is_some() {
                        counters.coalesced.fetch_add(1, Ordering::Relaxed);
                    }
                    counters.pending.store(pending.len() as u64, Ordering::Relaxed);
                }
                Some(Command::Shutdown(ack)) => {
                    flush(&pool, &mut pending, &counters).await;
                    let _ = ack.send(());
                    break;
                }
                None => {
                    flush(&pool, &mut pending, &counters).await;
                    break;
                }
            },
            _ = ticker.tick() => {
                flush(&pool, &mut pending, &counters).await;
            }
        }
    }
}

async fn flush(
    pool: &SqlitePool,
    pending: &mut HashMap<WriteKey, BufferedWrite>,
    counters: &Counters,
) {
    if pending.is_empty() {
        return;
    }
    let batch: Vec<BufferedWrite> = pending.drain().map(|(_, write)| write).collect();
    counters.pending.store(0, Ordering::Relaxed);
    match write_batch(pool, &batch).await {
        Ok(applied) => {
            counters
                .flushed
                .fetch_add(applied as u64, Ordering::Relaxed);
        }
        Err(error) => {
            counters.failed.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(error = %error, writes = batch.len(), "buffered heartbeat batch failed");
        }
    }
}

async fn write_batch(pool: &SqlitePool, batch: &[BufferedWrite]) -> sqlx::Result<usize> {
    let mut connection = pool.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut *connection)
        .await?;

    let mut applied = 0usize;
    for write in batch {
        let result = match write {
            BufferedWrite::JobHeartbeat { job_id } => {
                sqlx::query(HEARTBEAT_SQL)
                    .bind(job_id)
                    .execute(&mut *connection)
                    .await
            }
            BufferedWrite::NodeSeen { node_id } => {
                sqlx::query(NODE_SEEN_SQL)
                    .bind(node_id)
                    .execute(&mut *connection)
                    .await
            }
        };
        match result {
            Ok(done) => applied += done.rows_affected() as usize,
            Err(error) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *connection).await;
                return Err(error);
            }
        }
    }

    sqlx::query("COMMIT").execute(&mut *connection).await?;
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_updates_for_the_same_id_share_one_key() {
        let first = BufferedWrite::JobHeartbeat {
            job_id: "job-1".into(),
        };
        let second = BufferedWrite::JobHeartbeat {
            job_id: "job-1".into(),
        };
        let other = BufferedWrite::JobHeartbeat {
            job_id: "job-2".into(),
        };
        assert_eq!(first.key(), second.key());
        assert_ne!(first.key(), other.key());
        let node = BufferedWrite::NodeSeen {
            node_id: "job-1".into(),
        };
        assert_ne!(node.key(), first.key(), "job and node namespaces differ");
    }
}
