use crate::context::HitlChannelRequest;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HitlResolution {
    Approved,
    Rejected {
        reason: Option<String>,
    },
    ChoiceSelected {
        option_index: usize,
        option_value: String,
    },
    FeedbackProvided {
        feedback_text: String,
        attached_files: Vec<String>,
    },
    ArtifactEdited {
        path: String,
    },
}

/// Backwards-compatible alias for the channel payload.
pub type HitlRequestPayload = HitlChannelRequest;

#[derive(Debug, Clone)]
pub enum HitlOutcome {
    Resolved(HitlResolution),
    Expired,
    Cancelled,
}
