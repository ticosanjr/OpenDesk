use opendesk_domain::ChannelCapabilities;
use serde::Serialize;

/// Descriptor for a channel enabled at compile time. The I/O adapters still
/// live in the shell until the engine/storage extraction completes; the
/// feature flags already decide which channels the V2 process registers.
#[derive(Debug, Clone, Serialize)]
pub struct ChannelDescriptor {
    pub id: &'static str,
    pub capabilities: ChannelCapabilities,
}

#[allow(clippy::vec_init_then_push, unused_mut)] // every push is gated by a feature flag
pub fn enabled_channels() -> Vec<ChannelDescriptor> {
    let mut channels = Vec::new();
    #[cfg(feature = "ch-telegram")]
    channels.push(ChannelDescriptor {
        id: "telegram",
        capabilities: ChannelCapabilities {
            threads: false,
            attachments: true,
            buttons: true,
            media: true,
        },
    });
    #[cfg(feature = "ch-whatsapp")]
    channels.push(ChannelDescriptor {
        id: "whatsapp",
        capabilities: ChannelCapabilities {
            threads: false,
            attachments: true,
            buttons: true,
            media: true,
        },
    });
    #[cfg(feature = "ch-email")]
    channels.push(ChannelDescriptor {
        id: "email",
        capabilities: ChannelCapabilities {
            threads: true,
            attachments: true,
            buttons: false,
            media: false,
        },
    });
    #[cfg(feature = "ch-signal")]
    channels.push(ChannelDescriptor {
        id: "signal",
        capabilities: ChannelCapabilities {
            threads: false,
            attachments: true,
            buttons: true,
            media: true,
        },
    });
    #[cfg(feature = "ch-webpush")]
    channels.push(ChannelDescriptor {
        id: "webpush",
        capabilities: ChannelCapabilities {
            threads: false,
            attachments: false,
            buttons: true,
            media: false,
        },
    });
    channels
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_matches_the_compiled_features() {
        let ids = enabled_channels()
            .into_iter()
            .map(|channel| channel.id)
            .collect::<Vec<_>>();

        #[cfg(feature = "ch-telegram")]
        assert!(ids.contains(&"telegram"));
        #[cfg(not(feature = "ch-telegram"))]
        assert!(!ids.contains(&"telegram"));
        #[cfg(feature = "ch-email")]
        assert!(ids.contains(&"email"));
        #[cfg(feature = "ch-webpush")]
        assert!(ids.contains(&"webpush"));

        #[cfg(all(
            feature = "ch-telegram",
            feature = "ch-whatsapp",
            feature = "ch-email",
            feature = "ch-signal",
            feature = "ch-webpush"
        ))]
        assert_eq!(ids.len(), 5);
    }
}
