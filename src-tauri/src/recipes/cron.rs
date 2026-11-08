use crate::{db::schedules, error::AppResult, recipes::parser::Trigger};
use chrono::{DateTime, Utc};

pub const DEFAULT_CRON_EXPRESSION: &str = "0 8 * * *";

/// The normalized CRON contract shared by the calendar and the recipe runtime.
///
/// A trigger without config still resolves to a daily slot, and a missing
/// timezone follows the system instead of a hardcoded city.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronTrigger<'a> {
    pub expression: &'a str,
    pub timezone: String,
}

impl<'a> CronTrigger<'a> {
    pub fn from_trigger(trigger: &'a Trigger) -> Option<Self> {
        if trigger.trigger_type != "CRON" {
            return None;
        }
        let expression = trigger
            .config
            .get("cron")
            .or_else(|| trigger.config.get("cron_expression"))
            .or_else(|| trigger.config.get("schedule"))
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_CRON_EXPRESSION);
        let timezone = trigger
            .config
            .get("timezone")
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(schedules::default_timezone);
        Some(Self {
            expression,
            timezone,
        })
    }

    pub fn next_occurrence(&self, after: DateTime<Utc>) -> AppResult<DateTime<Utc>> {
        crate::queue_scheduler::cron_engine::next_occurrence(self.expression, &self.timezone, after)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn trigger(config: serde_json::Value) -> Trigger {
        Trigger {
            trigger_type: "CRON".into(),
            config,
        }
    }

    #[test]
    fn ignores_non_cron_triggers() {
        let trigger = Trigger {
            trigger_type: "MANUAL".into(),
            config: json!({}),
        };
        assert!(CronTrigger::from_trigger(&trigger).is_none());
    }

    #[test]
    fn accepts_every_supported_expression_key() {
        for key in ["cron", "cron_expression", "schedule"] {
            let trigger = trigger(json!({ key: "0 9 * * *" }));
            assert_eq!(
                CronTrigger::from_trigger(&trigger).map(|cron| cron.expression),
                Some("0 9 * * *")
            );
        }
    }

    #[test]
    fn empty_config_falls_back_to_the_daily_slot() {
        let trigger = trigger(json!({}));
        let cron = CronTrigger::from_trigger(&trigger).expect("cron trigger");
        assert_eq!(cron.expression, DEFAULT_CRON_EXPRESSION);
        assert_eq!(cron.timezone, schedules::default_timezone());
    }

    #[test]
    fn keeps_an_explicit_timezone() {
        let trigger = trigger(json!({ "cron": "0 9 * * *", "timezone": "UTC" }));
        let cron = CronTrigger::from_trigger(&trigger).expect("cron trigger");
        assert_eq!(cron.timezone, "UTC");
    }
}
