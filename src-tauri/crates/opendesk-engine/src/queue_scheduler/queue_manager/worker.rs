use super::QueueManager;
use crate::{error::AppResult, queue_scheduler::leases};
use opendesk_storage::events::OpenDeskWsEvent;
use opendesk_storage::repos::{budget, queue, runs};

use std::sync::{Arc, atomic::Ordering};

impl QueueManager {
    pub(super) async fn worker_loop(self: Arc<Self>) {
        if let Err(error) = self.recover().await {
            tracing::error!(error = %error, "queue recovery failed");
        }

        loop {
            tokio::select! {
                _ = self.shutdown.cancelled() => break,
                _ = self.notify.notified() => {}
                // Wake at the nearest lease; 30 s is only a safety net.
                _ = leases::next_wake(&self.pool) => {}
            }

            self.expire_leases().await;

            if self.shutdown.is_cancelled() || self.is_paused() {
                continue;
            }

            // Fill every available concurrency slot before going back to
            // sleep. Each job runs in its own task so a slow model call does
            // not block the next queued job.
            let policy = self.policy();
            loop {
                if self.active.load(Ordering::Relaxed) >= self.concurrency() {
                    break;
                }

                match budget::check(&self.pool).await {
                    Ok(check) if !check.allowed => {
                        self.pause(check.message.clone()).await;
                        self.events.publish(OpenDeskWsEvent::BudgetAlert {
                            level: check.level.into(),
                            daily_spent: check.status.daily_spent,
                            daily_limit: check.status.daily_limit,
                            message: check.message,
                        });
                        break;
                    }
                    Ok(_) => {}
                    Err(error) => {
                        tracing::warn!(error = %error, "could not check budget");
                    }
                }

                let Some(job) = (match queue::next_eligible(&self.pool, &policy).await {
                    Ok(job) => job,
                    Err(error) => {
                        tracing::error!(error = %error, "could not read queue");
                        None
                    }
                }) else {
                    break;
                };

                if !queue::claim(&self.pool, &job.id).await.unwrap_or(false) {
                    continue;
                }

                let workspace_id = job.workspace_id.clone();
                let manager = Arc::clone(&self);
                let active = self.active.fetch_add(1, Ordering::Relaxed) + 1;
                self.daemon.set_active_jobs(active).await;
                tokio::spawn(async move {
                    manager.execute_job(job).await;
                    let remaining = manager
                        .active
                        .fetch_sub(1, Ordering::Relaxed)
                        .saturating_sub(1);
                    manager.daemon.set_active_jobs(remaining).await;
                    manager.publish_queue(&workspace_id).await;
                    manager.notify.notify_waiters();
                });
            }
        }
    }

    async fn expire_leases(&self) {
        let Ok(jobs) = queue::expired_leases(&self.pool).await else {
            return;
        };
        for job in jobs {
            self.lease_expired
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .insert(job.id.clone());
            if let Some(token) = self
                .running_jobs
                .lock()
                .unwrap_or_else(|poison| poison.into_inner())
                .get(&job.id)
            {
                token.cancel();
            }
            tracing::warn!(job = %job.id, "queue lease expired; interrupting job");
        }
    }

    async fn recover(&self) -> AppResult<()> {
        let runs = runs::recover_running(&self.pool).await?;
        let jobs = queue::recover_running(&self.pool).await?;
        if runs > 0 || jobs > 0 {
            let _ = sqlx::query(
                "UPDATE tasks SET status = 'BLOCKED', phase_status = 'failed',
                    blocked_reason = 'Execução interrompida por reinício do OpenDesk',
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 WHERE id IN (
                    SELECT task_id FROM execution_queue
                    WHERE task_id IS NOT NULL
                      AND error_message LIKE 'interrupted by OpenDesk restart%'
                 )",
            )
            .execute(&self.pool)
            .await;
            tracing::warn!(
                runs,
                jobs,
                "recovered interrupted jobs after restart; manual retry required"
            );
        }
        self.notify.notify_waiters();
        Ok(())
    }
}
