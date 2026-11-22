use crate::{
    error::{DomainError, DomainResult},
    ids::WorkspaceId,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    pub name: String,
    #[serde(default)]
    pub mime: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

impl Attachment {
    pub fn validate(&self) -> DomainResult<()> {
        if self.name.trim().is_empty() {
            return Err(DomainError::validation("anexo sem nome"));
        }
        if self.path.is_none() && self.url.is_none() {
            return Err(DomainError::validation(format!(
                "anexo {} sem path/url",
                self.name
            )));
        }
        if self.path.is_some() && self.url.is_some() {
            return Err(DomainError::validation(format!(
                "anexo {} tem path e url; use um",
                self.name
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelThread {
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default)]
    pub parent_id: Option<String>,
}

/// Provider-agnostic message produced by an inbound channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedEnvelope {
    pub workspace_id: WorkspaceId,
    pub channel: String,
    pub external_id: String,
    pub sender: Option<String>,
    pub text: String,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub thread: ChannelThread,
    /// Stable idempotency key. Derived from the channel/external id when the
    /// adapter does not supply one.
    pub dedupe_key: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl NormalizedEnvelope {
    pub fn dedupe_key_for(channel: &str, external_id: &str, thread: &ChannelThread) -> String {
        format!(
            "{channel}:{external_id}:{}",
            thread.parent_id.as_deref().unwrap_or("-")
        )
    }

    pub fn validate(&self) -> DomainResult<()> {
        if self.workspace_id.is_empty() {
            return Err(DomainError::validation("envelope sem workspace"));
        }
        if self.channel.trim().is_empty() {
            return Err(DomainError::validation("envelope sem canal"));
        }
        if self.external_id.trim().is_empty() {
            return Err(DomainError::validation("envelope sem external_id"));
        }
        if self.dedupe_key.trim().is_empty() {
            return Err(DomainError::validation("envelope sem dedupe_key"));
        }
        if self.text.trim().is_empty() && self.attachments.is_empty() {
            return Err(DomainError::validation("envelope vazio"));
        }
        for attachment in &self.attachments {
            attachment.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelCapabilities {
    pub threads: bool,
    pub attachments: bool,
    pub buttons: bool,
    pub media: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryReceipt {
    pub delivered: bool,
    pub external_id: Option<String>,
}

/// Port implemented by every channel adapter (Telegram, e-mail, Signal…).
/// The engine never names a provider; it only depends on this trait.
#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    fn channel_id(&self) -> &'static str;
    fn capabilities(&self) -> ChannelCapabilities;
    async fn send(&self, envelope: &NormalizedEnvelope) -> DomainResult<DeliveryReceipt>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope() -> NormalizedEnvelope {
        NormalizedEnvelope {
            workspace_id: WorkspaceId::from("ws-1"),
            channel: "email".into(),
            external_id: "msg-1".into(),
            sender: None,
            text: "oi".into(),
            attachments: vec![],
            thread: ChannelThread::default(),
            dedupe_key: NormalizedEnvelope::dedupe_key_for(
                "email",
                "msg-1",
                &ChannelThread::default(),
            ),
            metadata: serde_json::Value::Null,
        }
    }

    #[test]
    fn envelope_requires_content_and_identity() {
        let valid = envelope();
        assert!(valid.validate().is_ok());

        let mut empty = envelope();
        empty.text.clear();
        assert!(empty.validate().is_err());

        let mut no_key = envelope();
        no_key.dedupe_key.clear();
        assert!(no_key.validate().is_err());
    }

    #[test]
    fn attachments_need_exactly_one_location() {
        let valid = Attachment {
            name: "a.pdf".into(),
            mime: Some("application/pdf".into()),
            path: Some("/tmp/a.pdf".into()),
            url: None,
        };
        assert!(valid.validate().is_ok());
        let both = Attachment {
            url: Some("https://x/a.pdf".into()),
            ..valid.clone()
        };
        assert!(both.validate().is_err());
        let neither = Attachment {
            path: None,
            url: None,
            ..valid
        };
        assert!(neither.validate().is_err());
    }

    #[test]
    fn dedupe_key_includes_the_thread() {
        let parent = ChannelThread {
            parent_id: Some("root".into()),
            external_id: None,
        };
        assert_eq!(
            NormalizedEnvelope::dedupe_key_for("email", "m1", &parent),
            "email:m1:root"
        );
    }
}
