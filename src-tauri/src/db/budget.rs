use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BudgetSettings {
    pub daily_limit_usd: f64,
    pub monthly_limit_usd: f64,
    pub soft_alert_threshold_pct: f64,
    pub hard_stop_enabled: bool,
    pub opencode_go_5h_token_limit: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateBudgetInput {
    pub daily_limit_usd: Option<f64>,
    pub monthly_limit_usd: Option<f64>,
    pub soft_alert_threshold_pct: Option<f64>,
    pub hard_stop_enabled: Option<bool>,
    pub opencode_go_5h_token_limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BudgetStatus {
    pub daily_spent: f64,
    pub monthly_spent: f64,
    pub five_hour_tokens: i64,
    pub daily_limit: f64,
    pub monthly_limit: f64,
    pub five_hour_limit: i64,
    pub daily_pct: f64,
    pub monthly_pct: f64,
    pub soft_alert: bool,
    pub hard_stop: bool,
    pub hard_stop_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct BudgetCheck {
    pub allowed: bool,
    pub level: &'static str,
    pub message: String,
    pub status: BudgetStatus,
}

pub async fn get(pool: &SqlitePool) -> AppResult<BudgetSettings> {
    sqlx::query_as::<_, BudgetSettings>(
        "SELECT daily_limit_usd, monthly_limit_usd, soft_alert_threshold_pct,
            hard_stop_enabled, opencode_go_5h_token_limit, updated_at
         FROM budget_settings WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Internal("budget_settings row is missing".into()))
}

pub async fn update(pool: &SqlitePool, input: UpdateBudgetInput) -> AppResult<BudgetSettings> {
    let current = get(pool).await?;
    let daily = input.daily_limit_usd.unwrap_or(current.daily_limit_usd);
    let monthly = input.monthly_limit_usd.unwrap_or(current.monthly_limit_usd);
    let soft = input
        .soft_alert_threshold_pct
        .unwrap_or(current.soft_alert_threshold_pct);
    let five_hour = input
        .opencode_go_5h_token_limit
        .unwrap_or(current.opencode_go_5h_token_limit);

    if daily <= 0.0 || monthly <= 0.0 || five_hour < 0 {
        return Err(AppError::BadRequest(
            "budget limits must be positive numbers".into(),
        ));
    }
    if !(0.0..=100.0).contains(&soft) {
        return Err(AppError::BadRequest(
            "soft_alert_threshold_pct must be between 0 and 100".into(),
        ));
    }

    sqlx::query(
        "UPDATE budget_settings SET
            daily_limit_usd = ?, monthly_limit_usd = ?, soft_alert_threshold_pct = ?,
            hard_stop_enabled = ?, opencode_go_5h_token_limit = ?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = 1",
    )
    .bind(daily)
    .bind(monthly)
    .bind(soft)
    .bind(input.hard_stop_enabled.unwrap_or(current.hard_stop_enabled))
    .bind(five_hour)
    .execute(pool)
    .await?;

    get(pool).await
}

pub async fn status(pool: &SqlitePool) -> AppResult<BudgetStatus> {
    let settings = get(pool).await?;

    let (daily_spent, monthly_spent): (f64, f64) = sqlx::query_as(
        "SELECT
            COALESCE(SUM(CASE WHEN date(started_at) = date('now') THEN estimated_cost_usd ELSE 0 END), 0.0),
            COALESCE(SUM(CASE WHEN strftime('%Y-%m', started_at) = strftime('%Y-%m', 'now')
                THEN estimated_cost_usd ELSE 0 END), 0.0)
         FROM task_runs
         WHERE status IN ('SUCCESS', 'FAILED')",
    )
    .fetch_one(pool)
    .await?;

    let five_hour_tokens: i64 = sqlx::query_scalar(
        "SELECT COALESCE(SUM(tokens_input + tokens_output), 0)
         FROM task_runs
         WHERE status IN ('SUCCESS', 'FAILED')
           AND datetime(started_at) >= datetime('now', '-5 hours')",
    )
    .fetch_one(pool)
    .await?;

    let daily_pct = if settings.daily_limit_usd > 0.0 {
        (daily_spent / settings.daily_limit_usd) * 100.0
    } else {
        0.0
    };
    let monthly_pct = if settings.monthly_limit_usd > 0.0 {
        (monthly_spent / settings.monthly_limit_usd) * 100.0
    } else {
        0.0
    };

    let soft_alert = daily_pct >= settings.soft_alert_threshold_pct
        || monthly_pct >= settings.soft_alert_threshold_pct;
    let hard_stop = settings.hard_stop_enabled
        && (daily_spent >= settings.daily_limit_usd
            || monthly_spent >= settings.monthly_limit_usd
            || (settings.opencode_go_5h_token_limit > 0
                && five_hour_tokens >= settings.opencode_go_5h_token_limit));

    Ok(BudgetStatus {
        daily_spent,
        monthly_spent,
        five_hour_tokens,
        daily_limit: settings.daily_limit_usd,
        monthly_limit: settings.monthly_limit_usd,
        five_hour_limit: settings.opencode_go_5h_token_limit,
        daily_pct,
        monthly_pct,
        soft_alert,
        hard_stop,
        hard_stop_enabled: settings.hard_stop_enabled,
    })
}

pub async fn check(pool: &SqlitePool) -> AppResult<BudgetCheck> {
    let status = status(pool).await?;
    if status.hard_stop {
        let message = if status.daily_spent >= status.daily_limit {
            format!(
                "hard cap diário atingido: ${:.2} / ${:.2}",
                status.daily_spent, status.daily_limit
            )
        } else if status.monthly_spent >= status.monthly_limit {
            format!(
                "hard cap mensal atingido: ${:.2} / ${:.2}",
                status.monthly_spent, status.monthly_limit
            )
        } else {
            format!(
                "limite de 5h do opencode-go atingido: {} / {} tokens",
                status.five_hour_tokens, status.five_hour_limit
            )
        };
        return Ok(BudgetCheck {
            allowed: false,
            level: "hard",
            message,
            status,
        });
    }

    if status.soft_alert {
        return Ok(BudgetCheck {
            allowed: true,
            level: "soft",
            message: format!(
                "orçamento em {:.0}% do diário e {:.0}% do mensal",
                status.daily_pct, status.monthly_pct
            ),
            status,
        });
    }

    Ok(BudgetCheck {
        allowed: true,
        level: "ok",
        message: "orçamento dentro do limite".into(),
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, SqlitePool) {
        let temp = TempDir::new().expect("temp");
        let db_path = temp.path().join("budget.db");
        let pool = crate::db::init(&db_path).await.expect("db");
        sqlx::query("INSERT INTO workspaces (id, name, root_path) VALUES ('ws-1', 'WS', ?)")
            .bind(temp.path().to_string_lossy().to_string())
            .execute(&pool)
            .await
            .expect("workspace");
        (temp, pool)
    }

    async fn insert_run(pool: &SqlitePool, cost: f64, tokens: i64) {
        sqlx::query(
            "INSERT INTO task_runs (
                id, workspace_id, agent_name, execution_mode, model_id,
                effort_level, status, estimated_cost_usd, tokens_input, tokens_output,
                started_at
             ) VALUES (?, 'ws-1', 'build', 'build', 'opencode-go/glm-5.3',
                'high', 'SUCCESS', ?, ?, 0,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        )
        .bind(uuid::Uuid::now_v7().to_string())
        .bind(cost)
        .bind(tokens)
        .execute(pool)
        .await
        .expect("run");
    }

    #[tokio::test]
    async fn hard_stop_blocks_when_daily_limit_is_exceeded() {
        let (_temp, pool) = setup().await;
        insert_run(&pool, 0.02, 100).await;
        update(
            &pool,
            UpdateBudgetInput {
                daily_limit_usd: Some(0.01),
                monthly_limit_usd: None,
                soft_alert_threshold_pct: Some(80.0),
                hard_stop_enabled: Some(true),
                opencode_go_5h_token_limit: Some(1_000_000),
            },
        )
        .await
        .expect("budget");

        let status = status(&pool).await.expect("status");
        assert!(status.hard_stop);
        assert!((status.daily_spent - 0.02).abs() < f64::EPSILON);

        let check = check(&pool).await.expect("check");
        assert!(!check.allowed);
        assert_eq!(check.level, "hard");
    }

    #[tokio::test]
    async fn soft_alert_allows_the_job() {
        let (_temp, pool) = setup().await;
        insert_run(&pool, 0.85, 100).await;
        update(
            &pool,
            UpdateBudgetInput {
                daily_limit_usd: Some(1.0),
                monthly_limit_usd: Some(10.0),
                soft_alert_threshold_pct: Some(80.0),
                hard_stop_enabled: Some(true),
                opencode_go_5h_token_limit: None,
            },
        )
        .await
        .expect("budget");

        let check = check(&pool).await.expect("check");
        assert!(check.allowed);
        assert_eq!(check.level, "soft");
    }
}
