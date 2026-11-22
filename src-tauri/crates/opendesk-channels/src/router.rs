use opendesk_domain::{CareLevel, NotificationLevel, QuietHours};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Channel routing policy. Adapters do not decide policy; the router does, and
/// the engine only emits a `NotificationIntent`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RouterConfig {
    /// Care level (`LOW`/`NORMAL`/`HIGH`/`CRITICAL`) → channel ids.
    #[serde(default)]
    pub channels_by_care: BTreeMap<String, Vec<String>>,
    /// Used when the care level has no explicit route.
    #[serde(default)]
    pub default_channels: Vec<String>,
    #[serde(default)]
    pub quiet_hours: Option<QuietHours>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotificationDecision {
    pub deliver: bool,
    pub deferred: bool,
    pub channels: Vec<String>,
    pub reason: &'static str,
}

pub struct NotificationRouter {
    config: RouterConfig,
}

impl NotificationRouter {
    pub fn new(config: RouterConfig) -> Self {
        Self { config }
    }

    /// `local_hour` is computed by the caller (workspace timezone). Critical
    /// alerts bypass quiet hours; everything else is deferred.
    pub fn route(
        &self,
        level: NotificationLevel,
        care: CareLevel,
        local_hour: u8,
    ) -> NotificationDecision {
        let quiet = self
            .config
            .quiet_hours
            .is_some_and(|hours| hours.contains(local_hour));
        if quiet && !matches!(level, NotificationLevel::Critical) {
            return NotificationDecision {
                deliver: false,
                deferred: true,
                channels: Vec::new(),
                reason: "quiet_hours",
            };
        }
        let channels = self
            .config
            .channels_by_care
            .get(care.as_str())
            .cloned()
            .unwrap_or_else(|| self.config.default_channels.clone());
        let mut unique = Vec::new();
        for channel in channels {
            let channel = channel.trim();
            if !channel.is_empty() && !unique.iter().any(|item| item == channel) {
                unique.push(channel.to_string());
            }
        }
        if unique.is_empty() {
            return NotificationDecision {
                deliver: false,
                deferred: false,
                channels: Vec::new(),
                reason: "no_channel",
            };
        }
        NotificationDecision {
            deliver: true,
            deferred: false,
            channels: unique,
            reason: "routed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> RouterConfig {
        let mut channels_by_care = BTreeMap::new();
        channels_by_care.insert("LOW".into(), vec!["email".into()]);
        channels_by_care.insert("CRITICAL".into(), vec!["signal".into(), "telegram".into()]);
        RouterConfig {
            channels_by_care,
            default_channels: vec!["desktop".into(), "desktop".into()],
            quiet_hours: Some(QuietHours::new(22, 7).expect("hours")),
        }
    }

    #[test]
    fn routes_by_care_and_dedupes_fallback() {
        let router = NotificationRouter::new(config());
        let low = router.route(NotificationLevel::Info, CareLevel::Low, 12);
        assert_eq!(low.channels, vec!["email"]);
        assert!(low.deliver);
        let normal = router.route(NotificationLevel::Info, CareLevel::Normal, 12);
        assert_eq!(normal.channels, vec!["desktop"]);
    }

    #[test]
    fn quiet_hours_defer_non_critical_but_not_critical() {
        let router = NotificationRouter::new(config());
        let normal = router.route(NotificationLevel::Warning, CareLevel::Normal, 23);
        assert!(!normal.deliver);
        assert!(normal.deferred);
        assert_eq!(normal.reason, "quiet_hours");
        let critical = router.route(NotificationLevel::Critical, CareLevel::Critical, 23);
        assert!(critical.deliver);
        assert_eq!(critical.channels, vec!["signal", "telegram"]);
    }

    #[test]
    fn reports_missing_channel() {
        let router = NotificationRouter::new(RouterConfig::default());
        let decision = router.route(NotificationLevel::Info, CareLevel::Normal, 12);
        assert!(!decision.deliver);
        assert_eq!(decision.reason, "no_channel");
    }
}
