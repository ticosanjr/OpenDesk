//! Shared CRON occurrence resolution.

use crate::error::{AppError, AppResult};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use croner::Cron;

pub fn next_occurrence(
    expression: &str,
    timezone: &str,
    after: DateTime<Utc>,
) -> AppResult<DateTime<Utc>> {
    let cron: Cron = expression
        .parse()
        .map_err(|error| AppError::BadRequest(format!("invalid cron expression: {error}")))?;
    let timezone: Tz = timezone
        .parse()
        .map_err(|_| AppError::BadRequest(format!("invalid timezone '{timezone}'")))?;
    let local = after.with_timezone(&timezone);
    let next = cron
        .find_next_occurrence(&local, false)
        .map_err(|error| AppError::BadRequest(format!("could not resolve next run: {error}")))?;
    Ok(next.with_timezone(&Utc))
}
