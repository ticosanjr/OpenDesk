//! Operational metrics derived from runs and queue history.

use crate::error::AppResult;
use chrono::DateTime;
use serde::Serialize;
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub days: i64,
    pub runs_total: i64,
    pub runs_success: i64,
    pub runs_failed: i64,
    pub runs_cancelled: i64,
    pub success_rate: f64,
    pub cost_usd: f64,
    pub tokens: i64,
    pub p50_duration_ms: i64,
    pub p95_duration_ms: i64,
    pub avg_duration_ms: i64,
    pub queue_samples: i64,
    pub queue_p50_wait_ms: i64,
    pub queue_p95_wait_ms: i64,
    pub queue_avg_wait_ms: i64,
    pub failure_codes: Vec<FailureCode>,
    pub agents: Vec<AgentMetrics>,
    pub daily: Vec<DailyMetrics>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FailureCode {
    pub code: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentMetrics {
    pub agent_id: String,
    pub runs: i64,
    pub success_rate: f64,
    pub cost_usd: f64,
    pub p95_duration_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyMetrics {
    pub date: String,
    pub runs: i64,
    pub failed: i64,
    pub cost_usd: f64,
}

#[derive(Debug, FromRow)]
struct RunRow {
    status: String,
    duration_ms: i64,
    estimated_cost_usd: f64,
    tokens: i64,
    agent_name: String,
    error_code: Option<String>,
}

#[derive(Debug, FromRow)]
struct QueueRow {
    enqueued_at: String,
    started_at: String,
}

#[derive(Debug, FromRow)]
struct DailyRow {
    date: String,
    runs: i64,
    failed: i64,
    cost_usd: f64,
}

fn percentile(values: &mut [i64], percentile: f64) -> i64 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    let index = ((values.len() - 1) as f64 * percentile).round() as usize;
    values[index.min(values.len() - 1)]
}

fn average(values: &[i64]) -> i64 {
    if values.is_empty() {
        return 0;
    }
    values.iter().sum::<i64>() / values.len() as i64
}

pub async fn collect(pool: &SqlitePool, workspace_id: &str, days: i64) -> AppResult<Metrics> {
    let days = days.clamp(1, 90);
    let runs: Vec<RunRow> = sqlx::query_as(
        "SELECT status, duration_ms, estimated_cost_usd,
                (tokens_input + tokens_output) AS tokens,
                agent_name, error_code
         FROM task_runs
         WHERE workspace_id = ?
           AND started_at >= strftime('%Y-%m-%dT%H:%M:%fZ','now', ?)",
    )
    .bind(workspace_id)
    .bind(format!("-{days} days"))
    .fetch_all(pool)
    .await?;

    let total = runs.len() as i64;
    let success = runs.iter().filter(|run| run.status == "SUCCESS").count() as i64;
    let failed = runs.iter().filter(|run| run.status == "FAILED").count() as i64;
    let cancelled = runs.iter().filter(|run| run.status == "CANCELLED").count() as i64;
    let cost_usd = runs.iter().map(|run| run.estimated_cost_usd).sum::<f64>();
    let tokens = runs.iter().map(|run| run.tokens).sum::<i64>();
    let mut durations = runs.iter().map(|run| run.duration_ms).collect::<Vec<_>>();
    let p50_duration_ms = percentile(&mut durations, 0.50);
    let p95_duration_ms = percentile(&mut durations, 0.95);
    let avg_duration_ms = average(&durations);

    let mut failures: std::collections::BTreeMap<String, i64> = Default::default();
    for run in &runs {
        if run.status != "FAILED" {
            continue;
        }
        let code = run.error_code.as_deref().unwrap_or("run_failed");
        *failures.entry(code.to_string()).or_default() += 1;
    }
    let mut failure_codes = failures
        .into_iter()
        .map(|(code, count)| FailureCode { code, count })
        .collect::<Vec<_>>();
    failure_codes.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then(left.code.cmp(&right.code))
    });

    let mut per_agent: std::collections::BTreeMap<String, Vec<&RunRow>> = Default::default();
    for run in &runs {
        per_agent
            .entry(run.agent_name.clone())
            .or_default()
            .push(run);
    }
    let mut agents = per_agent
        .into_iter()
        .map(|(agent_id, rows)| {
            let runs = rows.len() as i64;
            let success = rows.iter().filter(|run| run.status == "SUCCESS").count() as i64;
            let mut durations = rows.iter().map(|run| run.duration_ms).collect::<Vec<_>>();
            AgentMetrics {
                agent_id,
                runs,
                success_rate: if runs > 0 {
                    success as f64 / runs as f64
                } else {
                    0.0
                },
                cost_usd: rows.iter().map(|run| run.estimated_cost_usd).sum(),
                p95_duration_ms: percentile(&mut durations, 0.95),
            }
        })
        .collect::<Vec<_>>();
    agents.sort_by_key(|left| std::cmp::Reverse(left.runs));

    let queue_rows: Vec<QueueRow> = sqlx::query_as(
        "SELECT enqueued_at, started_at FROM execution_queue
         WHERE workspace_id = ?
           AND started_at IS NOT NULL
           AND enqueued_at >= strftime('%Y-%m-%dT%H:%M:%fZ','now', ?)",
    )
    .bind(workspace_id)
    .bind(format!("-{days} days"))
    .fetch_all(pool)
    .await?;
    let mut waits = queue_rows
        .iter()
        .filter_map(|row| {
            let enqueued = DateTime::parse_from_rfc3339(&row.enqueued_at).ok()?;
            let started = DateTime::parse_from_rfc3339(&row.started_at).ok()?;
            Some((started - enqueued).num_milliseconds().max(0))
        })
        .collect::<Vec<_>>();

    let daily_rows: Vec<DailyRow> = sqlx::query_as(
        "SELECT substr(started_at, 1, 10) AS date,
                COUNT(*) AS runs,
                COALESCE(SUM(status = 'FAILED'), 0) AS failed,
                CAST(COALESCE(SUM(estimated_cost_usd), 0) AS REAL) AS cost_usd
         FROM task_runs
         WHERE workspace_id = ?
           AND started_at >= strftime('%Y-%m-%dT%H:%M:%fZ','now', ?)
         GROUP BY substr(started_at, 1, 10)
         ORDER BY date",
    )
    .bind(workspace_id)
    .bind(format!("-{days} days"))
    .fetch_all(pool)
    .await?;

    Ok(Metrics {
        days,
        runs_total: total,
        runs_success: success,
        runs_failed: failed,
        runs_cancelled: cancelled,
        success_rate: if total > 0 {
            success as f64 / total as f64
        } else {
            0.0
        },
        cost_usd,
        tokens,
        p50_duration_ms,
        p95_duration_ms,
        avg_duration_ms,
        queue_samples: waits.len() as i64,
        queue_p50_wait_ms: percentile(&mut waits, 0.50),
        queue_p95_wait_ms: percentile(&mut waits, 0.95),
        queue_avg_wait_ms: average(&waits),
        failure_codes,
        agents,
        daily: daily_rows
            .into_iter()
            .map(|row| DailyMetrics {
                date: row.date,
                runs: row.runs,
                failed: row.failed,
                cost_usd: row.cost_usd,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support;

    #[test]
    fn percentiles_and_average_are_stable() {
        let mut values = vec![100, 200, 300, 400, 500];
        assert_eq!(percentile(&mut values, 0.5), 300);
        assert_eq!(percentile(&mut values, 0.95), 500);
        assert_eq!(average(&values), 300);
        assert_eq!(percentile(&mut [], 0.95), 0);
    }

    #[tokio::test]
    async fn collects_run_queue_and_failure_metrics() {
        let ctx = test_support::context().await;
        sqlx::query(
            "INSERT INTO task_runs (
                id, workspace_id, agent_name, execution_mode, model_id, effort_level,
                status, estimated_cost_usd, duration_ms, tokens_input, tokens_output,
                error_code, started_at
             ) VALUES
             ('r1', ?, 'build', 'build', 'm', 'high', 'SUCCESS', 1.0, 1000, 10, 20, NULL,
              strftime('%Y-%m-%dT%H:%M:%fZ','now','-2 days')),
             ('r2', ?, 'build', 'build', 'm', 'high', 'FAILED', 2.0, 3000, 30, 40, 'timeout',
              strftime('%Y-%m-%dT%H:%M:%fZ','now','-1 day')),
             ('r3', ?, 'plan', 'plan', 'm', 'low', 'SUCCESS', 0.5, 500, 5, 5, NULL,
              strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        )
        .bind(&ctx.workspace_id)
        .bind(&ctx.workspace_id)
        .bind(&ctx.workspace_id)
        .execute(&ctx.state.db)
        .await
        .expect("runs");
        sqlx::query(
            "INSERT INTO execution_queue (
                id, workspace_id, prompt_payload, resolved_mode, resolved_agent,
                resolved_model, resolved_effort, status, enqueued_at, started_at
             ) VALUES
             ('q1', ?, 'p', 'build', 'build', 'm', 'high', 'COMPLETED',
              strftime('%Y-%m-%dT%H:%M:%fZ','now','-1 hour'),
              strftime('%Y-%m-%dT%H:%M:%fZ','now','-59 minutes'))",
        )
        .bind(&ctx.workspace_id)
        .execute(&ctx.state.db)
        .await
        .expect("queue");

        let metrics = collect(&ctx.state.db, &ctx.workspace_id, 30)
            .await
            .expect("metrics");
        assert_eq!(metrics.runs_total, 3);
        assert_eq!(metrics.runs_failed, 1);
        assert!((metrics.success_rate - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(metrics.failure_codes[0].code, "timeout");
        assert_eq!(metrics.queue_samples, 1);
        assert!(metrics.queue_p95_wait_ms >= 59_000);
        assert_eq!(metrics.agents.len(), 2);
        assert_eq!(metrics.daily.len(), 3);
    }
}
