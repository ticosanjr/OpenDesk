use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use uuid::Uuid;

macro_rules! id_type {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// New time-ordered identifier.
            pub fn new() -> Self {
                Self(Uuid::now_v7().to_string())
            }

            pub fn from_string(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_string(self) -> String {
                self.0
            }

            pub fn is_empty(&self) -> bool {
                self.0.trim().is_empty()
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl FromStr for $name {
            type Err = crate::error::DomainError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                let value = value.trim();
                if value.is_empty() {
                    return Err(crate::error::DomainError::validation(concat!(
                        $label,
                        " cannot be empty"
                    )));
                }
                Ok(Self(value.to_string()))
            }
        }
    };
}

id_type!(WorkspaceId, "workspace id");
id_type!(TaskId, "task id");
id_type!(RunId, "run id");
id_type!(ConversationId, "conversation id");
id_type!(MessageId, "message id");
id_type!(ScheduleId, "schedule id");
id_type!(JobId, "job id");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_time_ordered_and_unique() {
        let first = TaskId::new();
        let second = TaskId::new();
        assert_ne!(first, second);
        assert!(first.as_str() < second.as_str());
        assert_eq!(first.to_string(), first.as_str());
    }

    #[test]
    fn empty_ids_are_rejected() {
        assert!("  ".parse::<RunId>().is_err());
        assert_eq!("run-1".parse::<RunId>().expect("id").as_str(), "run-1");
    }

    #[test]
    fn ids_round_trip_through_serde() {
        let id = ConversationId::from("conv-1");
        let json = serde_json::to_string(&id).expect("serialize");
        assert_eq!(json, "\"conv-1\"");
        let back: ConversationId = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(id, back);
    }
}
