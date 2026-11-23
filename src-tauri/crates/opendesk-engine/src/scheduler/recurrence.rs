//! Shared CRON occurrence resolution.

use crate::error::{AppError, AppResult};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
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

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recurrence {
    /// once | hourly | daily | weekly | weekdays | monthly | custom
    pub frequency: String,
    #[serde(default = "default_interval")]
    pub interval: u32,
    /// HH:MM in the schedule timezone.
    #[serde(default = "default_time")]
    pub time: String,
    /// 1 = Monday ... 7 = Sunday.
    #[serde(default)]
    pub weekdays: Vec<u8>,
    #[serde(default)]
    pub day_of_month: Option<u8>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub cron_expression: Option<String>,
}

fn default_interval() -> u32 {
    1
}

fn default_time() -> String {
    "08:00".into()
}

impl Default for Recurrence {
    fn default() -> Self {
        Self {
            frequency: "daily".into(),
            interval: 1,
            time: "08:00".into(),
            weekdays: Vec::new(),
            day_of_month: None,
            start_date: None,
            end_date: None,
            cron_expression: None,
        }
    }
}

impl Recurrence {
    pub fn to_cron(&self) -> AppResult<String> {
        let (minute, hour) = parse_time(&self.time)?;
        let expression = match self.frequency.as_str() {
            "once" => {
                let date = self.start_date.as_deref().ok_or_else(|| {
                    AppError::BadRequest("once recurrence requires a start date".into())
                })?;
                let date = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| {
                    AppError::BadRequest("invalid start date; use YYYY-MM-DD".into())
                })?;
                format!("{minute} {hour} {} {} *", date.day(), date.month())
            }
            "hourly" => format!("{minute} */{} * * *", self.interval.clamp(1, 23)),
            "daily" => format!("{minute} {hour} * * *"),
            "weekly" => {
                let days = if self.weekdays.is_empty() {
                    vec![1, 2, 3, 4, 5, 6, 7]
                } else {
                    self.weekdays.clone()
                };
                let days = days
                    .into_iter()
                    .filter(|day| (1..=7).contains(day))
                    .map(|day| if day == 7 { 0 } else { u32::from(day) })
                    .collect::<Vec<u32>>();
                format!("{minute} {hour} * * {}", join_days(&days))
            }
            "weekdays" => format!("{minute} {hour} * * 1-5"),
            "monthly" => {
                let day = self.day_of_month.unwrap_or(1).clamp(1, 31);
                format!("{minute} {hour} {day} * *")
            }
            "custom" => self
                .cron_expression
                .clone()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    AppError::BadRequest("custom recurrence requires a cron expression".into())
                })?,
            other => {
                return Err(AppError::BadRequest(format!(
                    "invalid recurrence frequency '{other}'"
                )));
            }
        };
        // Validate using the same parser the scheduler uses.
        let _: croner::Cron = expression
            .parse()
            .map_err(|error| AppError::BadRequest(format!("invalid cron expression: {error}")))?;
        Ok(expression)
    }

    pub fn human(&self) -> String {
        let time = self.time.clone();
        match self.frequency.as_str() {
            "once" => format!(
                "uma vez em {} às {}",
                self.start_date.as_deref().unwrap_or("data não definida"),
                time
            ),
            "hourly" => format!(
                "a cada {} hora(s), no minuto {}",
                self.interval,
                minute_of(&time)
            ),
            "daily" => format!("todos os dias às {time}"),
            "weekly" => {
                let names = self
                    .weekdays
                    .iter()
                    .map(|day| weekday_name(*day))
                    .collect::<Vec<_>>()
                    .join(", ");
                if names.is_empty() {
                    format!("todas as semanas às {time}")
                } else {
                    format!("{names} às {time}")
                }
            }
            "weekdays" => format!("dias úteis às {time}"),
            "monthly" => format!(
                "todo dia {} de cada mês às {time}",
                self.day_of_month.unwrap_or(1)
            ),
            "custom" => self
                .cron_expression
                .clone()
                .unwrap_or_else(|| "cron personalizado".into()),
            _ => "recorrência inválida".into(),
        }
    }

    pub fn once_datetime(&self, timezone: &str) -> AppResult<DateTime<Utc>> {
        let date = self
            .start_date
            .as_deref()
            .ok_or_else(|| AppError::BadRequest("once recurrence requires a start date".into()))?;
        let date = NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map_err(|_| AppError::BadRequest("invalid start date; use YYYY-MM-DD".into()))?;
        let (hour, minute) = self
            .time
            .split_once(':')
            .ok_or_else(|| AppError::BadRequest("time must be HH:MM".into()))?;
        let time = chrono::NaiveTime::from_hms_opt(
            hour.parse().unwrap_or(0),
            minute.parse().unwrap_or(0),
            0,
        )
        .ok_or_else(|| AppError::BadRequest("invalid time".into()))?;
        let naive = date.and_time(time);
        let tz: chrono_tz::Tz = timezone
            .parse()
            .map_err(|_| AppError::BadRequest(format!("invalid timezone '{timezone}'")))?;
        use chrono::TimeZone;
        tz.from_local_datetime(&naive)
            .single()
            .map(|value| value.with_timezone(&Utc))
            .ok_or_else(|| AppError::BadRequest("could not resolve the local once datetime".into()))
    }

    pub fn is_expired(&self, now: DateTime<Utc>, timezone: &str) -> bool {
        let Some(end) = self.end_date.as_deref() else {
            return false;
        };
        let Ok(end) = NaiveDate::parse_from_str(end, "%Y-%m-%d") else {
            return false;
        };
        let tz: chrono_tz::Tz = timezone.parse().unwrap_or(chrono_tz::UTC);
        now.with_timezone(&tz).date_naive() > end
    }
}

pub fn parse_recurrence(value: Option<&str>) -> Option<Recurrence> {
    value.and_then(|value| serde_json::from_str(value).ok())
}

pub fn next_runs(
    expression: &str,
    timezone: &str,
    after: DateTime<Utc>,
    count: usize,
) -> AppResult<Vec<String>> {
    let mut values = Vec::new();
    let mut cursor = after;
    for _ in 0..count.clamp(1, 20) {
        let next = next_occurrence(expression, timezone, cursor)?;
        values.push(next.to_rfc3339());
        cursor = next;
    }
    Ok(values)
}

fn parse_time(value: &str) -> AppResult<(u32, u32)> {
    let (hour, minute) = value
        .split_once(':')
        .ok_or_else(|| AppError::BadRequest("time must be HH:MM".into()))?;
    let hour: u32 = hour
        .parse()
        .map_err(|_| AppError::BadRequest("invalid hour".into()))?;
    let minute: u32 = minute
        .parse()
        .map_err(|_| AppError::BadRequest("invalid minute".into()))?;
    if hour > 23 || minute > 59 {
        return Err(AppError::BadRequest(
            "time must be between 00:00 and 23:59".into(),
        ));
    }
    Ok((minute, hour))
}

fn minute_of(value: &str) -> u32 {
    value
        .split_once(':')
        .and_then(|(_, minute)| minute.parse().ok())
        .unwrap_or(0)
}

fn join_days(days: &[u32]) -> String {
    days.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn weekday_name(day: u8) -> &'static str {
    match day {
        1 => "segunda",
        2 => "terça",
        3 => "quarta",
        4 => "quinta",
        5 => "sexta",
        6 => "sábado",
        7 => "domingo",
        _ => "dia inválido",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn daily() -> Recurrence {
        Recurrence {
            frequency: "daily".into(),
            time: "08:30".into(),
            ..Default::default()
        }
    }

    #[test]
    fn translates_common_recurrences() {
        assert_eq!(daily().to_cron().unwrap(), "30 8 * * *");
        assert_eq!(
            Recurrence {
                frequency: "weekdays".into(),
                time: "07:00".into(),
                ..Default::default()
            }
            .to_cron()
            .unwrap(),
            "0 7 * * 1-5"
        );
        assert_eq!(
            Recurrence {
                frequency: "weekly".into(),
                time: "09:00".into(),
                weekdays: vec![1, 3, 7],
                ..Default::default()
            }
            .to_cron()
            .unwrap(),
            "0 9 * * 1,3,0"
        );
        assert_eq!(
            Recurrence {
                frequency: "hourly".into(),
                time: "00:15".into(),
                interval: 6,
                ..Default::default()
            }
            .to_cron()
            .unwrap(),
            "15 */6 * * *"
        );
        assert_eq!(
            Recurrence {
                frequency: "monthly".into(),
                time: "10:00".into(),
                day_of_month: Some(5),
                ..Default::default()
            }
            .to_cron()
            .unwrap(),
            "0 10 5 * *"
        );
    }

    #[test]
    fn validates_time_and_custom_cron() {
        let invalid = Recurrence {
            time: "25:00".into(),
            ..daily()
        };
        assert!(invalid.to_cron().is_err());
        let custom = Recurrence {
            frequency: "custom".into(),
            cron_expression: Some("0 8 * * 1".into()),
            ..Default::default()
        };
        assert_eq!(custom.to_cron().unwrap(), "0 8 * * 1");
        let invalid_cron = Recurrence {
            frequency: "custom".into(),
            cron_expression: Some("nonsense".into()),
            ..Default::default()
        };
        assert!(invalid_cron.to_cron().is_err());
    }

    #[test]
    fn explains_recurrence_in_portuguese() {
        assert!(daily().human().contains("todos os dias"));
        assert!(
            Recurrence {
                frequency: "weekdays".into(),
                time: "07:00".into(),
                ..Default::default()
            }
            .human()
            .contains("dias úteis")
        );
    }

    #[test]
    fn computes_next_runs() {
        let runs = next_runs("0 8 * * *", "UTC", Utc::now(), 3).expect("runs");
        assert_eq!(runs.len(), 3);
    }

    #[test]
    fn resolves_once_datetime_in_local_timezone() {
        let recurrence = Recurrence {
            frequency: "once".into(),
            time: "09:30".into(),
            start_date: Some("2026-12-24".into()),
            ..Default::default()
        };
        let value = recurrence
            .once_datetime("America/Sao_Paulo")
            .expect("datetime");
        assert_eq!(value.to_rfc3339(), "2026-12-24T12:30:00+00:00");
        assert_eq!(recurrence.to_cron().unwrap(), "30 9 24 12 *");
    }

    #[test]
    fn detects_expired_and_open_ended_recurrences() {
        let now = DateTime::parse_from_rfc3339("2026-11-06T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let expired = Recurrence {
            frequency: "daily".into(),
            end_date: Some("2026-11-01".into()),
            ..Default::default()
        };
        assert!(expired.is_expired(now, "America/Sao_Paulo"));

        let still_running = Recurrence {
            frequency: "daily".into(),
            end_date: Some("2026-11-30".into()),
            ..Default::default()
        };
        assert!(!still_running.is_expired(now, "America/Sao_Paulo"));

        let open_ended = Recurrence {
            frequency: "daily".into(),
            ..Default::default()
        };
        assert!(!open_ended.is_expired(now, "America/Sao_Paulo"));
    }
}
