use crate::error::{AppError, AppResult};
use crate::repos::settings;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::collections::HashMap;

use super::{COLUMNS, QueueJob};

pub const QUEUE_POLICY_SETTING: &str = "queue_policy";

/// Capacity policy persisted in `app_settings` and shared by the worker, the
/// API and the UI. `max_concurrency` is the global slot budget; group and
/// agent limits are additional gates.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueuePolicy {
    pub max_concurrency: usize,
    pub max_queue_size: usize,
    pub default_agent_concurrency: usize,
    pub group_limits: HashMap<String, usize>,
}

impl Default for QueuePolicy {
    fn default() -> Self {
        let mut group_limits = HashMap::new();
        group_limits.insert("local_llm_heavy".into(), 1);
        group_limits.insert("web_scraper".into(), 2);
        group_limits.insert("light_api".into(), 5);
        Self {
            max_concurrency: 3,
            max_queue_size: 200,
            default_agent_concurrency: 2,
            group_limits,
        }
    }
}

impl QueuePolicy {
    pub fn normalized(mut self) -> Self {
        self.max_concurrency = self.max_concurrency.clamp(1, 32);
        self.max_queue_size = self.max_queue_size.clamp(1, 10_000);
        self.default_agent_concurrency = self.default_agent_concurrency.clamp(1, 32);
        self.group_limits.retain(|key, _| !key.trim().is_empty());
        for limit in self.group_limits.values_mut() {
            *limit = (*limit).clamp(1, 32);
        }
        self
    }

    pub fn group_limit(&self, group: &str) -> usize {
        self.group_limits
            .get(group)
            .or_else(|| self.group_limits.get("default"))
            .copied()
            .unwrap_or(self.max_concurrency)
    }

    pub fn validate(&self) -> AppResult<()> {
        if !(1..=32).contains(&self.max_concurrency) {
            return Err(AppError::BadRequest(
                "max_concurrency deve ficar entre 1 e 32".into(),
            ));
        }
        if !(1..=10_000).contains(&self.max_queue_size) {
            return Err(AppError::BadRequest(
                "max_queue_size deve ficar entre 1 e 10000".into(),
            ));
        }
        if !(1..=32).contains(&self.default_agent_concurrency) {
            return Err(AppError::BadRequest(
                "default_agent_concurrency deve ficar entre 1 e 32".into(),
            ));
        }
        if self
            .group_limits
            .values()
            .any(|limit| !(1..=32).contains(limit))
        {
            return Err(AppError::BadRequest(
                "limites de grupo devem ficar entre 1 e 32".into(),
            ));
        }
        Ok(())
    }
}

pub async fn load_policy(pool: &SqlitePool) -> AppResult<QueuePolicy> {
    let stored = settings::get(pool, QUEUE_POLICY_SETTING).await?;
    Ok(stored
        .and_then(|value| serde_json::from_value::<QueuePolicy>(value).ok())
        .unwrap_or_default()
        .normalized())
}

pub async fn save_policy(pool: &SqlitePool, policy: &QueuePolicy) -> AppResult<QueuePolicy> {
    let policy = policy.clone().normalized();
    policy.validate()?;
    settings::set(
        pool,
        QUEUE_POLICY_SETTING,
        &serde_json::to_value(&policy).map_err(|error| AppError::Internal(error.to_string()))?,
    )
    .await?;
    Ok(policy)
}

/// Claims the next job whose group, agent and task/session gates all have a
/// free slot. A blocked high-priority group never hides an eligible job from
/// another group (head-of-line blocking is resolved in SQL).
pub async fn next_eligible(pool: &SqlitePool, policy: &QueuePolicy) -> AppResult<Option<QueueJob>> {
    let mut groups = policy
        .group_limits
        .iter()
        .map(|(group, limit)| (group.clone(), (*limit).min(32) as i64))
        .collect::<Vec<_>>();
    groups.sort_by(|left, right| left.0.cmp(&right.0));

    let mut values = vec!["('default', ?)".to_string()];
    values.extend(groups.iter().map(|_| "(?, ?)".to_string()));
    let query = format!(
        "WITH limits(group_name, max_slots) AS (VALUES {})
         SELECT {COLUMNS} FROM execution_queue q
         LEFT JOIN tasks t ON t.id = q.task_id
         WHERE q.status = 'QUEUED'
           AND (
             SELECT COUNT(*) FROM execution_queue r
             WHERE r.status = 'RUNNING' AND r.concurrency_group = q.concurrency_group
           ) < COALESCE(
             (SELECT max_slots FROM limits WHERE group_name = q.concurrency_group),
             (SELECT max_slots FROM limits WHERE group_name = 'default'),
             ?
           )
           AND (
             SELECT COUNT(*) FROM execution_queue r
             WHERE r.status = 'RUNNING'
               AND r.workspace_id = q.workspace_id
               AND r.resolved_agent = q.resolved_agent
           ) < COALESCE(
             NULLIF((
               SELECT p.concurrency_quota FROM agent_profiles p
               WHERE p.workspace_id = q.workspace_id
                 AND p.agent_id = q.resolved_agent
                 AND p.concurrency_quota > 0
               LIMIT 1
             ), 0),
             ?
           )
           AND NOT EXISTS (
             SELECT 1 FROM execution_queue r
             WHERE r.status = 'RUNNING' AND r.id <> q.id AND (
               (q.task_id IS NOT NULL AND r.task_id = q.task_id)
               OR (q.session_id IS NOT NULL AND r.session_id = q.session_id)
             )
           )
         ORDER BY q.priority DESC, q.enqueued_at ASC
         LIMIT 1",
        values.join(", ")
    );
    let mut statement =
        sqlx::query_as::<_, QueueJob>(&query).bind(policy.group_limit("default") as i64);
    for (group, limit) in &groups {
        statement = statement.bind(group).bind(limit);
    }
    statement = statement
        .bind(policy.max_concurrency as i64)
        .bind(policy.default_agent_concurrency as i64);
    Ok(statement.fetch_optional(pool).await?)
}

pub async fn next_queued(pool: &SqlitePool) -> AppResult<Option<QueueJob>> {
    let policy = load_policy(pool).await?;
    next_eligible(pool, &policy).await
}

/// Explains why each waiting job is not running. The map only contains jobs
/// that are actually waiting; running and terminal jobs are absent.
fn scheduled_in_future(at: &str, now: &str) -> bool {
    match (
        chrono::DateTime::parse_from_rfc3339(at),
        chrono::DateTime::parse_from_rfc3339(now),
    ) {
        (Ok(at), Ok(now)) => at > now,
        _ => false,
    }
}

pub async fn wait_reasons(
    pool: &SqlitePool,
    jobs: &[QueueJob],
    policy: &QueuePolicy,
    paused: bool,
) -> AppResult<HashMap<String, String>> {
    let mut reasons = HashMap::new();
    if jobs.is_empty() {
        return Ok(reasons);
    }
    let group_counts = sqlx::query_as::<_, (String, i64)>(
        "SELECT COALESCE(concurrency_group, 'default'), COUNT(*)
         FROM execution_queue WHERE status = 'RUNNING'
         GROUP BY 1",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect::<HashMap<_, _>>();
    let agent_counts = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT workspace_id, resolved_agent, COUNT(*)
         FROM execution_queue WHERE status = 'RUNNING'
         GROUP BY 1, 2",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(workspace, agent, count)| (format!("{workspace}:{agent}"), count))
    .collect::<HashMap<_, _>>();
    let agent_quotas = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT workspace_id, agent_id, concurrency_quota FROM agent_profiles
         WHERE concurrency_quota > 0",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|(workspace, agent, quota)| (format!("{workspace}:{agent}"), quota as usize))
    .collect::<HashMap<_, _>>();
    let running_tasks = sqlx::query_scalar::<_, String>(
        "SELECT task_id FROM execution_queue
         WHERE status = 'RUNNING' AND task_id IS NOT NULL",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect::<std::collections::HashSet<_>>();
    let running_sessions = sqlx::query_scalar::<_, String>(
        "SELECT session_id FROM execution_queue
         WHERE status = 'RUNNING' AND session_id IS NOT NULL",
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .collect::<std::collections::HashSet<_>>();
    let now: String = sqlx::query_scalar("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')")
        .fetch_one(pool)
        .await?;

    for job in jobs {
        if !matches!(job.status.as_str(), "QUEUED" | "PAUSED") {
            continue;
        }
        let reason = if paused {
            "Fila pausada".to_string()
        } else if job.status == "PAUSED" {
            "Job pausado".to_string()
        } else if job
            .scheduled_for
            .as_deref()
            .is_some_and(|at| scheduled_in_future(at, &now))
        {
            "Aguardando horário agendado".to_string()
        } else if job
            .task_id
            .as_deref()
            .is_some_and(|task| running_tasks.contains(task))
        {
            "Mesma tarefa já em execução".to_string()
        } else if job
            .session_id
            .as_deref()
            .is_some_and(|session| running_sessions.contains(session))
        {
            "Mesma sessão já em execução".to_string()
        } else {
            let group = job.concurrency_group.as_deref().unwrap_or("default");
            let limit = policy.group_limit(group);
            let active = group_counts.get(group).copied().unwrap_or(0);
            if active >= limit as i64 {
                format!("Grupo {group} cheio ({active}/{limit})")
            } else {
                let key = format!("{}:{}", job.workspace_id, job.resolved_agent);
                let active = agent_counts.get(&key).copied().unwrap_or(0);
                let quota = agent_quotas
                    .get(&key)
                    .copied()
                    .unwrap_or(policy.default_agent_concurrency);
                if active >= quota as i64 {
                    format!("Agente {} cheio ({active}/{quota})", job.resolved_agent)
                } else {
                    "Aguardando slot livre".to_string()
                }
            }
        };
        reasons.insert(job.id.clone(), reason);
    }
    Ok(reasons)
}
