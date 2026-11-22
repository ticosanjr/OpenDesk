use crate::error::{DomainError, DomainResult};
use serde::{Deserialize, Serialize};

/// HITL 2.0 vocabulary shared by task interactions and recipe checkpoints.
///
/// Legacy recipe names are accepted on input so existing YAML keeps working;
/// everything new is written as one of the four canonical kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HitlKind {
    Approval,
    SingleChoice,
    MultiChoice,
    Text,
}

impl HitlKind {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_uppercase().as_str() {
            "APPROVAL"
            | "BINARY_GATE"
            | "APPROVAL_WITH_FEEDBACK"
            | "ARTIFACT_EDIT_HANDOFF"
            | "TOOL_PERMISSION_PROXY"
            | "ACKNOWLEDGE"
            | "ARTIFACT_REVIEW" => Some(Self::Approval),
            "SINGLE_CHOICE" => Some(Self::SingleChoice),
            "MULTI_CHOICE" => Some(Self::MultiChoice),
            "TEXT" | "FEEDBACK" => Some(Self::Text),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approval => "APPROVAL",
            Self::SingleChoice => "SINGLE_CHOICE",
            Self::MultiChoice => "MULTI_CHOICE",
            Self::Text => "TEXT",
        }
    }

    /// Reads a stored value and repairs rows that have options but a TEXT kind.
    pub fn effective(value: &str, options: &[String], multiple: bool) -> Self {
        let parsed = Self::parse(value).unwrap_or(Self::Text);
        if parsed == Self::Text && !options.is_empty() {
            Self::from_options(options, multiple)
        } else {
            parsed
        }
    }

    /// Derives the kind from the legacy `options`/`multiple` pair.
    pub fn from_options(options: &[String], multiple: bool) -> Self {
        if options.is_empty() {
            Self::Text
        } else if multiple {
            Self::MultiChoice
        } else {
            Self::SingleChoice
        }
    }

    pub fn requires_options(self) -> bool {
        matches!(self, Self::SingleChoice | Self::MultiChoice)
    }
}

/// Persisted interaction state (`task_interactions.status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HitlStatus {
    Pending,
    Resolved,
    Expired,
    Cancelled,
}

impl HitlStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Resolved => "RESOLVED",
            Self::Expired => "EXPIRED",
            Self::Cancelled => "CANCELLED",
        }
    }

    pub fn parse(value: &str) -> DomainResult<Self> {
        match value.trim().to_uppercase().as_str() {
            "PENDING" => Ok(Self::Pending),
            "RESOLVED" => Ok(Self::Resolved),
            "EXPIRED" => Ok(Self::Expired),
            "CANCELLED" | "CANCELED" => Ok(Self::Cancelled),
            other => Err(DomainError::validation(format!(
                "status de HITL inválido '{other}'"
            ))),
        }
    }

    pub fn is_pending(self) -> bool {
        matches!(self, Self::Pending)
    }
}

/// Decision attached to a resolved interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HitlOutcome {
    Approved,
    Rejected,
    Answered,
}

impl HitlOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approved => "APPROVED",
            Self::Rejected => "REJECTED",
            Self::Answered => "ANSWERED",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_legacy_recipe_types_to_the_canonical_kinds() {
        assert_eq!(HitlKind::parse("BINARY_GATE"), Some(HitlKind::Approval));
        assert_eq!(
            HitlKind::parse("APPROVAL_WITH_FEEDBACK"),
            Some(HitlKind::Approval)
        );
        assert_eq!(
            HitlKind::parse("ARTIFACT_EDIT_HANDOFF"),
            Some(HitlKind::Approval)
        );
        assert_eq!(HitlKind::parse("MULTI_CHOICE"), Some(HitlKind::MultiChoice));
        assert_eq!(
            HitlKind::parse("SINGLE_CHOICE"),
            Some(HitlKind::SingleChoice)
        );
        assert_eq!(HitlKind::parse("TEXT"), Some(HitlKind::Text));
        assert_eq!(HitlKind::parse("nope"), None);
    }

    #[test]
    fn derives_kind_from_legacy_options() {
        assert_eq!(HitlKind::from_options(&[], false), HitlKind::Text);
        assert_eq!(
            HitlKind::from_options(&["A".into()], false),
            HitlKind::SingleChoice
        );
        assert_eq!(
            HitlKind::from_options(&["A".into()], true),
            HitlKind::MultiChoice
        );
    }

    #[test]
    fn status_round_trips() {
        for status in [
            HitlStatus::Pending,
            HitlStatus::Resolved,
            HitlStatus::Expired,
            HitlStatus::Cancelled,
        ] {
            assert_eq!(HitlStatus::parse(status.as_str()).expect("status"), status);
        }
        assert!(HitlStatus::Pending.is_pending());
        assert!(!HitlStatus::Resolved.is_pending());
    }
}
