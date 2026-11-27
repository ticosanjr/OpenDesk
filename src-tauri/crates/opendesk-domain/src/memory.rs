//! Memory usage scope shared by storage, engine and the shell.

use crate::error::DomainError;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Where a personal rule or structured note may be injected.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryUsageScope {
    Normal,
    Chat,
    #[default]
    Both,
}

impl MemoryUsageScope {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Chat => "chat",
            Self::Both => "both",
        }
    }

    /// Normal executions (tasks, routines, recipes) read `normal` and `both`.
    pub fn allows_normal(self) -> bool {
        matches!(self, Self::Normal | Self::Both)
    }

    /// Chat executions read `chat` and `both`.
    pub fn allows_chat(self) -> bool {
        matches!(self, Self::Chat | Self::Both)
    }
}

impl FromStr for MemoryUsageScope {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_lowercase().as_str() {
            "normal" => Ok(Self::Normal),
            "chat" => Ok(Self::Chat),
            "both" => Ok(Self::Both),
            other => Err(DomainError::Validation(format!(
                "usage_scope inválido '{other}'; use 'normal', 'chat' ou 'both'"
            ))),
        }
    }
}

impl std::fmt::Display for MemoryUsageScope {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_the_three_documented_values() {
        assert_eq!(
            "normal".parse::<MemoryUsageScope>().unwrap(),
            MemoryUsageScope::Normal
        );
        assert_eq!(
            "CHAT".parse::<MemoryUsageScope>().unwrap(),
            MemoryUsageScope::Chat
        );
        assert_eq!(
            " both ".parse::<MemoryUsageScope>().unwrap(),
            MemoryUsageScope::Both
        );
        assert!("none".parse::<MemoryUsageScope>().is_err());
        assert!(MemoryUsageScope::Normal.allows_normal());
        assert!(!MemoryUsageScope::Normal.allows_chat());
        assert!(MemoryUsageScope::Both.allows_chat());
    }
}
