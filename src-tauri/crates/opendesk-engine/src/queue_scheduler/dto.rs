//! Queue job DTO conversion used by API handlers and WebSocket events.

use opendesk_storage::events::QueueJobDto;
use opendesk_storage::repos::queue;

pub fn job_dto(job: queue::QueueJob) -> QueueJobDto {
    QueueJobDto {
        id: job.id,
        task_id: job.task_id,
        title: job.task_title,
        status: job.status,
        priority: job.priority as i32,
        concurrency_group: job.concurrency_group,
        lease_expires_at: job.lease_expires_at,
        last_heartbeat_at: job.last_heartbeat_at,
        resolved_mode: job.resolved_mode,
        resolved_agent: job.resolved_agent,
        resolved_model: job.resolved_model,
        resolved_effort: job.resolved_effort,
        job_kind: job.job_kind,
        output_dir: job.output_dir,
        scheduled_for: job.scheduled_for,
        enqueued_at: job.enqueued_at,
        started_at: job.started_at,
        finished_at: job.finished_at,
        retry_count: job.retry_count,
        max_retries: job.max_retries,
        error_message: job.error_message,
        task_origin: job.task_origin,
    }
}
