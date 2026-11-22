use crate::error::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};

/// How much the user cares about a flow. Drives channel routing in the
/// `NotificationRouter`; the engine only sets it, channels decide where it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CareLevel {
    Low,
    Normal,
    High,
    Critical,
}

impl CareLevel {
    pub const ALL: [Self; 4] = [Self::Low, Self::Normal, Self::High, Self::Critical];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "LOW",
            Self::Normal => "NORMAL",
            Self::High => "HIGH",
            Self::Critical => "CRITICAL",
        }
    }

    pub fn parse(value: &str) -> DomainResult<Self> {
        match value.trim().to_uppercase().as_str() {
            "LOW" => Ok(Self::Low),
            "NORMAL" | "MEDIUM" => Ok(Self::Normal),
            "HIGH" => Ok(Self::High),
            "CRITICAL" => Ok(Self::Critical),
            other => Err(DomainError::validation(format!(
                "care level inválido '{other}'"
            ))),
        }
    }

    pub fn is_critical(self) -> bool {
        matches!(self, Self::Critical)
    }
}

/// Local quiet window, in hours. `start == end` means "disabled".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuietHours {
    pub start_hour: u8,
    pub end_hour: u8,
}

impl QuietHours {
    pub fn new(start_hour: u8, end_hour: u8) -> DomainResult<Self> {
        if start_hour > 23 || end_hour > 23 {
            return Err(DomainError::validation("quiet hours fora de 0..23"));
        }
        Ok(Self {
            start_hour,
            end_hour,
        })
    }

    pub fn is_disabled(self) -> bool {
        self.start_hour == self.end_hour
    }

    /// Handles windows that cross midnight (22 → 7).
    pub fn contains(self, hour: u8) -> bool {
        if self.is_disabled() || hour > 23 {
            return false;
        }
        if self.start_hour < self.end_hour {
            hour >= self.start_hour && hour < self.end_hour
        } else {
            hour >= self.start_hour || hour < self.end_hour
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn care_levels_round_trip() {
        for level in CareLevel::ALL {
            assert_eq!(CareLevel::parse(level.as_str()).expect("level"), level);
        }
        assert_eq!(
            CareLevel::parse("medium").expect("alias"),
            CareLevel::Normal
        );
        assert!(CareLevel::parse("urgent").is_err());
        assert!(CareLevel::Critical.is_critical());
    }

    #[test]
    fn quiet_hours_cross_midnight() {
        let quiet = QuietHours::new(22, 7).expect("hours");
        assert!(quiet.contains(22));
        assert!(quiet.contains(23));
        assert!(quiet.contains(0));
        assert!(quiet.contains(6));
        assert!(!quiet.contains(7));
        assert!(!quiet.contains(12));
    }

    #[test]
    fn equal_hours_disable_the_window() {
        let quiet = QuietHours::new(8, 8).expect("hours");
        assert!(quiet.is_disabled());
        assert!(!quiet.contains(8));
        assert!(QuietHours::new(24, 7).is_err());
    }
}
